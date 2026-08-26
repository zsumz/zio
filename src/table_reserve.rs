//! Linear capacity and slot-reservation proofs.

use std::os::fd::BorrowedFd;

use crate::{
    Error, Interest, Key, Mode, RegistrationId,
    descriptor::Descriptor,
    token::{MAX_GENERATION, encode},
};

use super::{
    RegistrationTable,
    retire::RetireTicket,
    slot::{Entry, FREE_END, Slot},
};

/// Capacity proof consumed by one exact reservation.
pub(crate) struct ReservePermit<'table> {
    table: &'table mut RegistrationTable,
    slot: ReserveSlot,
}

enum ReserveSlot {
    Fresh { index: u32 },
    Reused { index: u32 },
}

impl<'table> ReservePermit<'table> {
    const fn fresh(table: &'table mut RegistrationTable, index: u32) -> Self {
        Self {
            table,
            slot: ReserveSlot::Fresh { index },
        }
    }

    const fn reused(table: &'table mut RegistrationTable, index: u32) -> Self {
        Self {
            table,
            slot: ReserveSlot::Reused { index },
        }
    }

    #[inline]
    pub(crate) fn reserve(
        self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Reservation<'table>, Error> {
        match self.slot {
            ReserveSlot::Fresh { index } => self
                .table
                .reserve_fresh(index, descriptor, key, interest, mode),
            ReserveSlot::Reused { index } => self
                .table
                .reserve_reused(index, descriptor, key, interest, mode),
        }
    }
}

/// Linear access to one newly inserted slot.
pub(crate) struct Reservation<'table> {
    table: &'table mut RegistrationTable,
    id: RegistrationId,
    retire: RetireTicket,
}

impl Reservation<'_> {
    pub(crate) const fn id(&self) -> RegistrationId {
        self.id
    }

    #[inline]
    pub(crate) fn descriptor(&self) -> Result<BorrowedFd<'_>, Error> {
        occupied_entry(self.table, self.retire.index()).map(|entry| entry.descriptor.as_fd())
    }

    #[inline]
    #[allow(
        clippy::unused_self,
        reason = "consuming the linear capability releases exclusive table access"
    )]
    pub(crate) fn keep<Value>(self, value: Value) -> Value {
        value
    }

    #[inline]
    pub(crate) fn retire(self) -> Result<(), Error> {
        self.table.commit_retire(self.retire)
    }

    #[inline]
    pub(crate) fn mark_uncertain(self) -> Result<(), Error> {
        self.table.commit_uncertain(self.retire)
    }
}

impl RegistrationTable {
    #[inline]
    fn reserve_fresh(
        &mut self,
        index: u32,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Reservation<'_>, Error> {
        let generation = core::num::NonZeroU32::new(1).ok_or(Error::Invariant)?;
        let id = encode(index, generation).ok_or(Error::RegistrationSpaceExhausted)?;
        let entry = Entry::registered(descriptor, key, interest, mode);
        self.slots.push(Slot::occupied(generation.get(), entry));
        let retire = RetireTicket::new(self.slots.len() - 1, index);
        Ok(Reservation {
            table: self,
            id,
            retire,
        })
    }

    #[inline]
    pub(crate) fn check_reservable(&mut self) -> Result<ReservePermit<'_>, Error> {
        if self.free_head != FREE_END {
            let index = self.free_head;
            let slot = self
                .slots
                .get(usize::try_from(index).map_err(|_| Error::Invariant)?)
                .ok_or(Error::Invariant)?;
            if slot.entry.is_some() || slot.generation == MAX_GENERATION {
                return Err(Error::Invariant);
            }
            return Ok(ReservePermit::reused(self, index));
        }
        if self.slots.len() < self.limit.get() {
            let index = u32::try_from(self.slots.len()).map_err(|_| Error::BackendOverflow)?;
            return Ok(ReservePermit::fresh(self, index));
        }
        if self.exhausted == self.limit.get() {
            Err(Error::RegistrationSpaceExhausted)
        } else {
            Err(Error::Capacity {
                limit: self.limit.get(),
            })
        }
    }

    #[inline]
    fn reserve_reused(
        &mut self,
        index: u32,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Reservation<'_>, Error> {
        let slot_index = usize::try_from(index).map_err(|_| Error::Invariant)?;
        let slot = self.slots.get_mut(slot_index).ok_or(Error::Invariant)?;
        let next_generation = slot
            .generation
            .checked_add(1)
            .ok_or(Error::RegistrationSpaceExhausted)?;
        let generation = core::num::NonZeroU32::new(next_generation).ok_or(Error::Invariant)?;
        let id = encode(index, generation).ok_or(Error::RegistrationSpaceExhausted)?;
        self.free_head = slot.next_free;
        slot.next_free = FREE_END;
        slot.generation = next_generation;
        slot.entry = Some(Entry::registered(descriptor, key, interest, mode));
        let retire = RetireTicket::new(slot_index, index);
        Ok(Reservation {
            table: self,
            id,
            retire,
        })
    }
}

#[inline]
fn occupied_entry(table: &RegistrationTable, index: usize) -> Result<&Entry, Error> {
    table
        .slots
        .get(index)
        .and_then(|slot| slot.entry.as_ref())
        .ok_or(Error::Invariant)
}
