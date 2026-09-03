//! Linear capacity and slot-reservation proofs.

use std::num::NonZeroU32;

use crate::{
    CapacityKind, CapacityReason, Error,
    token::{MAX_GENERATION, encode},
};

use super::{
    RegistrationTable,
    permit::{FreshPermit, ReusedPermit},
    slot::FREE_END,
};

impl RegistrationTable {
    #[inline]
    pub(crate) const fn has_reusable_slot(&self) -> bool {
        self.free_head != FREE_END
    }

    pub(crate) fn fresh_permit(&mut self) -> Result<FreshPermit<'_>, Error> {
        if self.has_reusable_slot() {
            return Err(Error::Invariant);
        }
        if self.slots.len() < self.limit.get() {
            let slot_index = self.slots.len();
            let free_index = u32::try_from(slot_index).map_err(|_| Error::Invariant)?;
            let id =
                encode(free_index, NonZeroU32::MIN).ok_or(Error::RegistrationSpaceExhausted)?;
            let Self {
                slots,
                free_head,
                exhausted,
                live,
                ..
            } = self;
            return Ok(FreshPermit {
                slots,
                free_head,
                exhausted,
                live,
                slot_index,
                free_index,
                id,
            });
        }
        if self.exhausted == self.limit.get() {
            Err(Error::RegistrationSpaceExhausted)
        } else {
            Err(Error::Capacity {
                kind: CapacityKind::Registration,
                limit: self.limit.get(),
                reason: CapacityReason::Exhausted,
            })
        }
    }

    #[inline]
    pub(crate) fn reused_permit(&mut self) -> Result<ReusedPermit<'_>, Error> {
        let free_index = self.free_head;
        if free_index == FREE_END {
            return Err(Error::Invariant);
        }
        let slot_index = usize::try_from(free_index).map_err(|_| Error::Invariant)?;
        let Self {
            slots,
            free_head,
            exhausted,
            live,
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
        Ok(ReusedPermit {
            slot,
            free_head,
            exhausted,
            live,
            free_index,
            next_free,
            next_generation,
            id,
        })
    }
}
