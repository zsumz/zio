//! Linear settlement for one validated registration generation.

use crate::{Error, RegistrationId, RegistrationState, binding::Binding, token::MAX_GENERATION};

use super::RegistrationTable;

/// Linear access to one validated live slot across a native mutation.
pub(crate) struct PreparedRetire<'table> {
    table: &'table mut RegistrationTable,
    retire: RetireTicket,
}

impl PreparedRetire<'_> {
    #[inline]
    pub(crate) fn binding(&self) -> Result<Binding<'_>, Error> {
        binding_at(self.table, self.retire.index())
    }

    #[inline]
    #[allow(
        clippy::unused_self,
        reason = "consuming the linear capability releases exclusive table access"
    )]
    pub(crate) fn keep(self) {}

    #[inline]
    pub(crate) fn retire(self) -> Result<(), Error> {
        self.table.commit_retire(self.retire)
    }

    #[inline]
    pub(crate) fn mark_uncertain(self) -> Result<(), Error> {
        self.table.commit_uncertain(self.retire)
    }
}

/// Opaque proof that one occupied slot was validated under exclusive access.
pub(super) struct RetireTicket {
    index: usize,
    free_index: u32,
}

impl RetireTicket {
    pub(super) const fn new(index: usize, free_index: u32) -> Self {
        Self { index, free_index }
    }

    pub(super) const fn index(&self) -> usize {
        self.index
    }

    const fn into_parts(self) -> (usize, u32) {
        (self.index, self.free_index)
    }
}

impl RegistrationTable {
    #[inline]
    pub(crate) fn prepare_retire(
        &mut self,
        id: RegistrationId,
        allow_uncertain: bool,
    ) -> Result<PreparedRetire<'_>, Error> {
        let (index, entry) = self.entry_with_index(id)?;
        let free_index = u32::try_from(index).map_err(|_| Error::Invariant)?;
        if !allow_uncertain && entry.state == RegistrationState::Uncertain {
            return Err(Error::Uncertain { registration: id });
        }
        Ok(PreparedRetire {
            table: self,
            retire: RetireTicket::new(index, free_index),
        })
    }

    #[inline]
    pub(super) fn commit_retire(&mut self, ticket: RetireTicket) -> Result<(), Error> {
        let (index, free_index) = ticket.into_parts();
        let slot = self.slots.get_mut(index).ok_or(Error::Invariant)?;
        if slot.generation == MAX_GENERATION {
            let exhausted = self.exhausted.checked_add(1).ok_or(Error::Invariant)?;
            slot.entry = None;
            slot.next_free = super::slot::FREE_END;
            self.exhausted = exhausted;
        } else {
            slot.entry = None;
            slot.next_free = self.free_head;
            self.free_head = free_index;
        }
        Ok(())
    }

    #[inline]
    pub(super) fn commit_uncertain(&mut self, ticket: RetireTicket) -> Result<(), Error> {
        let (index, _) = ticket.into_parts();
        let entry = self
            .slots
            .get_mut(index)
            .and_then(|slot| slot.entry.as_mut())
            .ok_or(Error::Invariant)?;
        entry.state = RegistrationState::Uncertain;
        Ok(())
    }
}

#[inline]
fn binding_at(table: &RegistrationTable, index: usize) -> Result<Binding<'_>, Error> {
    let entry = table
        .slots
        .get(index)
        .and_then(|slot| slot.entry.as_ref())
        .ok_or(Error::Invariant)?;
    Ok(Binding {
        descriptor: entry.descriptor.as_fd(),
        interest: entry.interest,
        mode: entry.mode,
        state: entry.state,
    })
}
