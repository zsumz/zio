//! Capacity diagnostic categories.

use core::fmt;

/// Storage category associated with a capacity failure.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CapacityKind {
    /// Delivered-event storage.
    Event,
    /// Retained-registration storage.
    Registration,
}

impl fmt::Display for CapacityKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Event => "event",
            Self::Registration => "registration",
        })
    }
}
