//! Registration-capacity representation checks.

use std::num::NonZeroUsize;

use crate::{CapacityKind, CapacityReason, Error, token::MAX_REGISTRATIONS};

use super::RegistrationTable;

impl RegistrationTable {
    pub(crate) const fn validate_capacity(limit: NonZeroUsize) -> Result<(), Error> {
        if limit.get() > MAX_REGISTRATIONS {
            return Err(Error::Capacity {
                kind: CapacityKind::Registration,
                limit: limit.get(),
                reason: CapacityReason::BackendLimit,
            });
        }
        Ok(())
    }
}
