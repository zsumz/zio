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

/// Why a fixed capacity was unavailable.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CapacityReason {
    /// The configured capacity was zero.
    Zero,
    /// No configured slot is currently reservable.
    Exhausted,
    /// Rust could not reserve the requested storage.
    StorageUnavailable,
    /// The configured capacity exceeds a backend representation limit.
    BackendLimit,
    /// Every configured slot exhausted its registration generations.
    GenerationExhausted,
}

impl fmt::Display for CapacityKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Event => "event",
            Self::Registration => "registration",
        })
    }
}

impl fmt::Display for CapacityReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Zero => "must be nonzero",
            Self::Exhausted => "is exhausted",
            Self::GenerationExhausted => "has no reusable generations",
            Self::StorageUnavailable => "could not be reserved",
            Self::BackendLimit => "exceeds the backend limit",
        })
    }
}
