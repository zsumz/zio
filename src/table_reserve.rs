//! Linear capacity and slot-reservation proofs.

use std::{num::NonZeroU32, os::fd::BorrowedFd};

use crate::{
    Error, Interest, Key, Mode, RegistrationId,
    descriptor::Descriptor,
    token::{MAX_GENERATION, encode},
};

use super::{
    RegistrationTable,
    retire::SlotLease,
    slot::{Entry, FREE_END, Slot},
};

/// Capacity proof retaining direct access to one exact vacant slot.
pub(crate) struct ReservePermit<'table> {
    slot: ReserveSlot<'table>,
    id: RegistrationId,
}

enum ReserveSlot<'table> {
    Fresh {
        slots: &'table mut Vec<Slot>,
        free_head: &'table mut u32,
        exhausted: &'table mut usize,
        slot_index: usize,
        free_index: u32,
    },
    Reused {
        slot: &'table mut Slot,
        free_head: &'table mut u32,
        exhausted: &'table mut usize,
        free_index: u32,
        next_free: u32,
        next_generation: u32,
    },
}

impl<'table> ReservePermit<'table> {
    pub(crate) const fn id(&self) -> RegistrationId {
        self.id
    }

    /// Inserts the descriptor, then lends that exact entry to one native call.
    ///
    /// The higher-ranked callback cannot return a borrow of the retained
    /// descriptor, so the resulting reservation remains its sole capability.
    #[inline]
    pub(crate) fn reserve_with<Output>(
        self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
        apply: impl for<'descriptor> FnOnce(BorrowedFd<'descriptor>, RegistrationId) -> Output,
    ) -> Result<(Reservation<'table>, Output), Error> {
        let id = self.id;
        match self.slot {
            ReserveSlot::Fresh {
                slots,
                free_head,
                exhausted,
                slot_index,
                free_index,
            } => {
                slots.push(Slot {
                    generation: 1,
                    entry: None,
                    next_free: FREE_END,
                });
                // `slot_index` was the length under this exclusive vector
                // borrow, and the immediately preceding push added that slot.
                let slot = slots.get_mut(slot_index).ok_or(Error::Invariant)?;
                let entry = slot
                    .entry
                    .insert(Entry::registered(descriptor, key, interest, mode));
                let output = apply(entry.descriptor.as_fd(), id);
                let lease = SlotLease::new(slot, free_head, exhausted, free_index);
                Ok((Reservation { lease }, output))
            }
            ReserveSlot::Reused {
                slot,
                free_head,
                exhausted,
                free_index,
                next_free,
                next_generation,
            } => {
                *free_head = next_free;
                slot.next_free = FREE_END;
                slot.generation = next_generation;
                let entry = slot
                    .entry
                    .insert(Entry::registered(descriptor, key, interest, mode));
                let output = apply(entry.descriptor.as_fd(), id);
                let lease = SlotLease::new(slot, free_head, exhausted, free_index);
                Ok((Reservation { lease }, output))
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn reserve(
        self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Reservation<'table>, Error> {
        self.reserve_with(descriptor, key, interest, mode, |_, _| ())
            .map(|(reservation, ())| reservation)
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
        self.lease.retire()
    }

    #[inline]
    pub(crate) fn mark_uncertain(self) -> Result<(), Error> {
        self.lease.mark_uncertain()
    }
}

impl RegistrationTable {
    #[inline]
    pub(crate) fn check_reservable(&mut self) -> Result<ReservePermit<'_>, Error> {
        if self.free_head != FREE_END {
            return self.check_reused();
        }
        if self.slots.len() < self.limit.get() {
            let slot_index = self.slots.len();
            let free_index = u32::try_from(slot_index).map_err(|_| Error::BackendOverflow)?;
            let id =
                encode(free_index, NonZeroU32::MIN).ok_or(Error::RegistrationSpaceExhausted)?;
            let Self {
                slots,
                free_head,
                exhausted,
                ..
            } = self;
            return Ok(ReservePermit {
                slot: ReserveSlot::Fresh {
                    slots,
                    free_head,
                    exhausted,
                    slot_index,
                    free_index,
                },
                id,
            });
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
    fn check_reused(&mut self) -> Result<ReservePermit<'_>, Error> {
        let free_index = self.free_head;
        let slot_index = usize::try_from(free_index).map_err(|_| Error::Invariant)?;
        let Self {
            slots,
            free_head,
            exhausted,
            ..
        } = self;
        let slot = slots.get_mut(slot_index).ok_or(Error::Invariant)?;
        if slot.entry.is_some() || slot.generation == MAX_GENERATION {
            return Err(Error::Invariant);
        }
        let next_generation = slot.generation.checked_add(1).ok_or(Error::Invariant)?;
        let generation = NonZeroU32::new(next_generation).ok_or(Error::Invariant)?;
        let id = encode(free_index, generation).ok_or(Error::RegistrationSpaceExhausted)?;
        let next_free = slot.next_free;
        Ok(ReservePermit {
            slot: ReserveSlot::Reused {
                slot,
                free_head,
                exhausted,
                free_index,
                next_free,
                next_generation,
            },
            id,
        })
    }
}
