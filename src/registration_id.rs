//! Registration identity interop.

use core::fmt;

use crate::RegistrationId;

impl fmt::Display for RegistrationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(formatter)
    }
}
