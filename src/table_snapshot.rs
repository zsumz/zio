//! Registration snapshot construction.

use std::num::NonZeroU32;

use crate::{
    CapacityKind, CapacityReason, Error, Registration, registration::PollId, token::encode,
};

use super::RegistrationTable;

impl RegistrationTable {
    pub(crate) fn snapshot(&self, owner: PollId) -> Result<Vec<Registration>, Error> {
        let occupied = self
            .slots
            .iter()
            .filter(|slot| slot.entry.is_some())
            .count();
        if occupied != self.live {
            return Err(Error::Invariant);
        }
        let mut registrations = Vec::new();
        registrations
            .try_reserve_exact(occupied)
            .map_err(|_| Error::Capacity {
                kind: CapacityKind::Registration,
                limit: occupied,
                reason: CapacityReason::StorageUnavailable,
            })?;
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.entry.is_none() {
                continue;
            }
            let index = u32::try_from(index).map_err(|_| Error::Invariant)?;
            let generation = NonZeroU32::new(slot.generation).ok_or(Error::Invariant)?;
            let id = encode(index, generation).ok_or(Error::Invariant)?;
            registrations.push(Registration::new(owner, id));
        }
        if registrations.len() != occupied {
            return Err(Error::Invariant);
        }
        Ok(registrations)
    }
}
