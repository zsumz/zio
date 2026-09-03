//! Registration-capacity representation checks.

use std::num::NonZeroUsize;

use crate::{CapacityKind, CapacityReason, Error, token::MAX_REGISTRATIONS};

use super::RegistrationTable;

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
}
