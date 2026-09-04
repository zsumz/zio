//! Cross-thread poller wake capability.

use std::fmt;

use crate::{Error, Key, Operation, sys::Wake};

/// Cloneable cross-thread capability for keyed [`crate::Event::Wake`] observations.
///
/// Clones may outlive the poller. They retain its native wake resource, not
/// registered descriptors; later triggers have no observer.
#[must_use = "retain the waker to signal the poller"]
#[derive(Clone)]
pub struct Waker {
    wake: Wake,
    key: Key,
}

impl Waker {
    pub(crate) fn new(wake: Wake, key: Key) -> Self {
        Self { wake, key }
    }

    /// Returns the key carried by this waker's observations.
    pub const fn key(&self) -> Key {
        self.key
    }

    /// Returns whether both capabilities produce the same keyed observation.
    pub fn will_wake(&self, other: &Self) -> bool {
        self.key == other.key && self.wake.same_target(&other.wake)
    }

    /// Requests the poller's configured wake observation.
    ///
    /// Wake delivery is a successful observation, not an interrupted-wait
    /// error. Multiple triggers may coalesce into one wake event; observations
    /// are notifications rather than a count of calls to this method.
    pub fn wake(&self) -> Result<(), Error> {
        self.wake.wake().map_err(|source| Error::Io {
            operation: Operation::TriggerWake,
            source,
        })
    }
}

impl fmt::Debug for Waker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Waker")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}
