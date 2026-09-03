//! Linear settlement for one validated registration generation.

use crate::{
    Error, RegistrationId, RegistrationState,
    binding::Binding,
    token::{EncodedRegistrationId, MAX_GENERATION},
};

#[cfg(test)]
use crate::token::decode;

use super::{RegistrationTable, slot::Slot};

/// Exclusive access to one occupied slot and its free-list state.
pub(super) struct SlotLease<'table> {
    slot: &'table mut Slot,
    free_head: &'table mut u32,
    exhausted: &'table mut usize,
    live: &'table mut usize,
    free_index: u32,
}

impl<'table> SlotLease<'table> {
    pub(super) const fn new(
        slot: &'table mut Slot,
        free_head: &'table mut u32,
        exhausted: &'table mut usize,
        live: &'table mut usize,
        free_index: u32,
    ) -> Self {
        Self {
            slot,
            free_head,
            exhausted,
            live,
            free_index,
        }
    }

    #[inline]
    fn binding(&self) -> Result<Binding<'_>, Error> {
        let entry = self.slot.entry.as_ref().ok_or(Error::Invariant)?;
        Ok(Binding {
            descriptor: entry.descriptor.as_fd(),
            interest: entry.interest,
            mode: entry.mode,
            state: entry.state,
        })
    }

    #[inline]
    #[allow(
        clippy::unused_self,
        reason = "consuming the linear capability releases exclusive slot access"
    )]
    pub(super) fn keep(self) {}

    #[inline]
    pub(super) fn retire(self) -> Result<(), Error> {
        let live = self.live.checked_sub(1).ok_or(Error::Invariant)?;
        if self.slot.generation == MAX_GENERATION {
            let exhausted = self.exhausted.checked_add(1).ok_or(Error::Invariant)?;
            self.slot.entry = None;
            self.slot.next_free = super::slot::FREE_END;
            *self.exhausted = exhausted;
        } else {
            self.slot.entry = None;
            self.slot.next_free = *self.free_head;
            *self.free_head = self.free_index;
        }
        *self.live = live;
        Ok(())
    }

    #[inline]
    pub(super) fn mark_uncertain(self) -> Result<(), Error> {
        let entry = self.slot.entry.as_mut().ok_or(Error::Invariant)?;
        entry.state = RegistrationState::Uncertain;
        Ok(())
    }
}

/// Linear access to one validated live slot across a native mutation.
pub(crate) struct PreparedRetire<'table> {
    lease: SlotLease<'table>,
}

impl PreparedRetire<'_> {
    #[inline]
    pub(crate) fn binding(&self) -> Result<Binding<'_>, Error> {
        self.lease.binding()
    }

    #[inline]
    #[allow(
        clippy::unused_self,
        reason = "consuming the linear capability releases exclusive table access"
    )]
    pub(crate) fn keep(self) {
        self.lease.keep();
    }

    #[inline]
    pub(crate) fn retire(self) -> Result<(), Error> {
        self.lease.retire()
    }

    #[inline]
    pub(crate) fn mark_uncertain(self) -> Result<(), Error> {
        self.lease.mark_uncertain()
    }
}

impl RegistrationTable {
    #[inline]
    pub(crate) fn prepare_registration_retire(
        &mut self,
        encoded: EncodedRegistrationId,
        allow_uncertain: bool,
    ) -> Result<PreparedRetire<'_>, Error> {
        let id = encoded.id();
        let (free_index, generation) = encoded.parts();
        let index = usize::try_from(free_index).map_err(|_| Error::Invariant)?;
        self.prepare_retire_at(id, index, free_index, generation, allow_uncertain)
    }

    #[cfg(test)]
    #[cfg_attr(
        not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )),
        allow(dead_code, reason = "matches supported retirement test support")
    )]
    #[inline]
    pub(crate) fn prepare_retire(
        &mut self,
        id: RegistrationId,
        allow_uncertain: bool,
    ) -> Result<PreparedRetire<'_>, Error> {
        let (index, generation) = decode(id)?;
        let free_index = u32::try_from(index).map_err(|_| Error::Invariant)?;
        self.prepare_retire_at(id, index, free_index, generation.get(), allow_uncertain)
    }

    #[inline]
    fn prepare_retire_at(
        &mut self,
        id: RegistrationId,
        index: usize,
        free_index: u32,
        generation: u32,
        allow_uncertain: bool,
    ) -> Result<PreparedRetire<'_>, Error> {
        let Self {
            slots,
            free_head,
            exhausted,
            live,
            ..
        } = self;
        let slot = slots
            .get_mut(index)
            .ok_or(Error::Stale { registration: id })?;
        if slot.generation != generation {
            return Err(Error::Stale { registration: id });
        }
        let entry = slot
            .entry
            .as_ref()
            .ok_or(Error::Stale { registration: id })?;
        if !allow_uncertain && entry.state == RegistrationState::Uncertain {
            return Err(Error::Uncertain { registration: id });
        }
        Ok(PreparedRetire {
            lease: SlotLease::new(slot, free_head, exhausted, live, free_index),
        })
    }
}
