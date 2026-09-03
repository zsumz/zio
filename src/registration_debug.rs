//! Backend-neutral registration diagnostics.

use core::fmt;

use crate::Registration;

impl fmt::Debug for Registration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Registration")
            .field("id", &self.id().get())
            .finish_non_exhaustive()
    }
}
