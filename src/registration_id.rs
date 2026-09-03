//! Registration identity interop.

use core::fmt;

use crate::RegistrationId;

impl From<RegistrationId> for u64 {
    fn from(registration: RegistrationId) -> Self {
        registration.get()
    }
}

impl fmt::Display for RegistrationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(formatter)
    }
}
