//! Linear capabilities for one reserved registration slot.

use std::os::fd::BorrowedFd;

use crate::{
    Error, Interest, Key, Mode, RegistrationId, descriptor::Descriptor,
    token::EncodedRegistrationId,
};

use super::{
    retire::SlotLease,
    slot::{Entry, FREE_END, Slot},
};

/// Capacity proof retaining direct access to one exact virgin slot.
pub(crate) struct FreshPermit<'table> {
    pub(super) slots: &'table mut Vec<Slot>,
    pub(super) free_head: &'table mut u32,
    pub(super) exhausted: &'table mut usize,
    pub(super) live: &'table mut usize,
    pub(super) slot_index: usize,
    pub(super) free_index: u32,
    pub(super) id: EncodedRegistrationId,
}

/// Capacity proof retaining direct access to one exact retired slot.
pub(crate) struct ReusedPermit<'table> {
    pub(super) slot: &'table mut Slot,
    pub(super) free_head: &'table mut u32,
    pub(super) exhausted: &'table mut usize,
    pub(super) live: &'table mut usize,
    pub(super) free_index: u32,
    pub(super) next_free: u32,
    pub(super) next_generation: u32,
    pub(super) id: EncodedRegistrationId,
}

impl<'table> FreshPermit<'table> {
    #[cfg(test)]
    pub(crate) const fn id(&self) -> RegistrationId {
        self.id.id()
    }

    pub(crate) const fn encoded_id(&self) -> EncodedRegistrationId {
        self.id
    }

    /// Inserts the descriptor, then lends that exact entry to one native call.
    #[inline]
    pub(crate) fn reserve_with<Output>(
        self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
        apply: impl for<'descriptor> FnOnce(BorrowedFd<'descriptor>, RegistrationId) -> Output,
    ) -> Result<(Reservation<'table>, Output), Error> {
        let Self {
            slots,
            free_head,
            exhausted,
            live,
            slot_index,
            free_index,
            id,
        } = self;
        let next_live = live.checked_add(1).ok_or(Error::Invariant)?;
        slots.push(Slot {
            generation: 1,
            entry: None,
            next_free: FREE_END,
        });
        let slot = slots.get_mut(slot_index).ok_or(Error::Invariant)?;
        let entry = slot
            .entry
            .insert(Entry::registered(descriptor, key, interest, mode));
        *live = next_live;
        let output = apply(entry.descriptor.as_fd(), id.id());
        let lease = SlotLease::new(slot, free_head, exhausted, live, free_index);
        Ok((Reservation { lease }, output))
    }
}

impl<'table> ReusedPermit<'table> {
    #[cfg(test)]
    pub(crate) const fn id(&self) -> RegistrationId {
        self.id.id()
    }

    pub(crate) const fn encoded_id(&self) -> EncodedRegistrationId {
        self.id
    }

    /// Inserts the descriptor, then lends that exact entry to one native call.
    #[inline]
    pub(crate) fn reserve_with<Output>(
        self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
        apply: impl for<'descriptor> FnOnce(BorrowedFd<'descriptor>, RegistrationId) -> Output,
    ) -> Result<(Reservation<'table>, Output), Error> {
        let Self {
            slot,
            free_head,
            exhausted,
            live,
            free_index,
            next_free,
            next_generation,
            id,
        } = self;
        let next_live = live.checked_add(1).ok_or(Error::Invariant)?;
        let entry = occupy(
            &mut slot.entry,
            Entry::registered(descriptor, key, interest, mode),
        )?;
        *free_head = next_free;
        slot.generation = next_generation;
        *live = next_live;
        let output = apply(entry.descriptor.as_fd(), id.id());
        let lease = SlotLease::new(slot, free_head, exhausted, live, free_index);
        Ok((Reservation { lease }, output))
    }
}

/// Linear access to one newly inserted slot.
pub(crate) struct Reservation<'table> {
    lease: SlotLease<'table>,
}

impl Reservation<'_> {
    #[inline]
    pub(crate) fn keep<Value>(self, value: Value) -> Value {
        self.lease.keep();
        value
    }

    #[inline]
    pub(crate) fn retire(self) -> Result<(), Error> {
        drop(self.release()?);
        Ok(())
    }

    #[inline]
    pub(crate) fn release(self) -> Result<Descriptor, Error> {
        self.lease.release()
    }

    #[inline]
    pub(crate) fn mark_uncertain(self) -> Result<(), Error> {
        self.lease.mark_uncertain()
    }
}

#[inline]
pub(super) fn occupy(vacancy: &mut Option<Entry>, entry: Entry) -> Result<&mut Entry, Error> {
    match vacancy {
        Some(_) => Err(reused_slot_occupied()),
        None => Ok(vacancy.insert(entry)),
    }
}

#[cold]
#[inline(never)]
const fn reused_slot_occupied() -> Error {
    Error::Invariant
}
