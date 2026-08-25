//! Maximum blocking behavior for one poll.

use core::time::Duration;

/// Maximum requested blocking behavior for one readiness observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wait {
    /// Return without intentionally blocking.
    NoBlock,
    /// Block for no longer than the supplied duration.
    For(Duration),
    /// Permit indefinite blocking until the backend returns.
    Forever,
}

impl Wait {
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
