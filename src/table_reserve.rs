//! Linear capacity and slot-reservation proofs.

use std::num::NonZeroU32;

use crate::{
    Error,
    token::{MAX_GENERATION, encode},
};

use super::{
    RegistrationTable,
    permit::{FreshPermit, ReusedPermit},
    slot::FREE_END,
};

impl RegistrationTable {
    #[inline]
    pub(crate) const fn has_virgin_slot(&self) -> bool {
        self.slots.len() < self.limit.get()
    }

    #[inline]
    pub(crate) const fn has_reusable_slot(&self) -> bool {
        self.free_head != FREE_END
    }

    pub(crate) fn fresh_permit(&mut self) -> Result<FreshPermit<'_>, Error> {
        if self.has_virgin_slot() {
            let slot_index = self.slots.len();
            let free_index = u32::try_from(slot_index).map_err(|_| Error::Invariant)?;
            let id = encode(free_index, NonZeroU32::MIN).ok_or(Error::Invariant)?;
            let Self {
                slots,
                free_head,
                free_tail,
                exhausted,
                live,
                ..
            } = self;
            return Ok(FreshPermit {
                slots,
                free_head,
                free_tail,
                exhausted,
                live,
                slot_index,
                free_index,
                id,
            });
        }
        if self.has_reusable_slot() {
            return Err(Error::Invariant);
        }
        self.ensure_reservable()?;
        Err(Error::Invariant)
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
            free_tail,
            exhausted,
            live,
            ..
        } = self;
        let slot = slots.get(slot_index).ok_or(Error::Invariant)?;
        if slot.entry.is_some() || slot.generation == MAX_GENERATION {
            return Err(Error::Invariant);
        }
        let next_generation = slot.generation.checked_add(1).ok_or(Error::Invariant)?;
        let generation = NonZeroU32::new(next_generation).ok_or(Error::Invariant)?;
        let id = encode(free_index, generation).ok_or(Error::Invariant)?;
        let next_free = slot.next_free;
        if *free_tail == FREE_END
            || (next_free == FREE_END && *free_tail != free_index)
            || (next_free != FREE_END && *free_tail == free_index)
        {
            return Err(Error::Invariant);
        }
        Ok(ReusedPermit {
            slots,
            free_head,
            free_tail,
            exhausted,
            live,
            slot_index,
            free_index,
            next_free,
            next_generation,
            id,
        })
    }
}
