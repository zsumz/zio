//! Requested blocking behavior for one poll.

use core::time::Duration;

/// Requested blocking behavior for one readiness observation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Wait {
    /// Return without intentionally blocking.
    NoBlock,
    /// Wait for readiness for approximately the supplied duration.
    ///
    /// A backend may round a positive duration up to its native timeout
    /// resolution. Zero remains nonblocking, and interruptions may return
    /// before the timeout elapses.
    For(Duration),
    /// Permit indefinite blocking until the backend returns.
    Forever,
}

impl Wait {
    /// Returns whether this request is nonblocking.
    pub const fn is_nonblocking(self) -> bool {
        match self {
            Self::NoBlock => true,
            Self::For(duration) => duration.is_zero(),
            Self::Forever => false,
        }
    }

    /// Returns the portable timeout representation.
    ///
    /// `None` represents an indefinite wait and zero represents a nonblocking
    /// observation.
    pub const fn timeout(self) -> Option<Duration> {
        match self {
            Self::NoBlock => Some(Duration::ZERO),
            Self::For(duration) => Some(duration),
            Self::Forever => None,
        }
    }
}

#[cfg(test)]
#[path = "wait_test.rs"]
mod tests;
