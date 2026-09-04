//! Registration-capacity representation checks.

use std::num::NonZeroUsize;

use crate::{CapacityKind, CapacityReason, Error, token::MAX_REGISTRATIONS};

use super::{RegistrationTable, slot::FREE_END};

impl RegistrationTable {
    pub(crate) const fn validate_capacity(limit: NonZeroUsize) -> Result<(), Error> {
        if limit.get().saturating_sub(MAX_REGISTRATIONS) != 0 {
            return Err(Error::Capacity {
                kind: CapacityKind::Registration,
                limit: limit.get(),
                reason: CapacityReason::BackendLimit,
            });
        }
        Ok(())
    }

    /// Proves that a registration can reserve either virgin or reusable space.
    pub(crate) const fn ensure_reservable(&self) -> Result<(), Error> {
        if self.slots.len() < self.limit.get() || self.free_head != FREE_END {
            return Ok(());
        }
        let reason = if self.exhausted == self.limit.get() {
            CapacityReason::GenerationExhausted
        } else {
            CapacityReason::Exhausted
        };
        Err(Error::Capacity {
            kind: CapacityKind::Registration,
            limit: self.limit.get(),
            reason,
        })
    }
}
