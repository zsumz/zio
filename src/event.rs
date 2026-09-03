//! Caller keys and portable readiness observations.
use crate::Registration;

/// Caller-selected value delivered with an observed event.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Key(u64);

impl Key {
    /// The zero key.
    pub const ZERO: Self = Self(0);

    /// Creates a caller key.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the stored value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Backend-neutral advisory readiness hints for one resource.
///
/// Hints are a snapshot of what the backend reported, not a promise that a
/// later operation will succeed or avoid blocking. Multiple hints may be
/// present. In particular, closure or error hints may accompany an event even
/// when the matching direction was not requested. Test flag membership and use
/// the corresponding nonblocking operation as the source of truth. A closure
/// hint identifies the operation direction to inspect, not the peer action that
/// caused it; native backends may conservatively report an additional hint, and
/// an absent closure hint does not prove that the direction remains open.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Readiness(u8);

impl Readiness {
    /// No readiness hints.
    pub const EMPTY: Self = Self(0);
    /// A read, receive, or accept operation may make progress.
    ///
    /// The operation can still race, return an error, or return stream EOF.
    pub const READABLE: Self = Self(1 << 0);
    /// A write, send, or pending connect operation may make progress.
    ///
    /// Inspect the operation result, including a socket's pending error when
    /// completing a nonblocking connect.
    pub const WRITABLE: Self = Self(1 << 1);
    /// The readable direction reported EOF or another terminal condition.
    ///
    /// Buffered data may remain, so this hint can accompany [`Self::READABLE`]
    /// before a zero-length stream read confirms EOF.
    pub const READ_CLOSED: Self = Self(1 << 2);
    /// The writable direction reported closure or terminal unavailability.
    ///
    /// The exact condition and peer cause remain resource- and
    /// platform-specific.
    pub const WRITE_CLOSED: Self = Self(1 << 3);
    /// The backend reported a resource-specific error hint.
    ///
    /// This flag contains no error code. Inspect the nonblocking operation and,
    /// for sockets where appropriate, the pending socket error.
    pub const ERROR: Self = Self(1 << 4);

    /// Returns whether no readiness hint is present.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns whether every flag in `other` is present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns the union of two readiness sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns whether readable readiness is present.
    pub const fn is_readable(self) -> bool {
        self.contains(Self::READABLE)
    }

    /// Returns whether writable readiness is present.
    pub const fn is_writable(self) -> bool {
        self.contains(Self::WRITABLE)
    }

    /// Returns whether readable closure is present.
    pub const fn is_read_closed(self) -> bool {
        self.contains(Self::READ_CLOSED)
    }

    /// Returns whether writable closure is present.
    pub const fn is_write_closed(self) -> bool {
        self.contains(Self::WRITE_CLOSED)
    }

    /// Returns whether an error hint is present.
    pub const fn is_error(self) -> bool {
        self.contains(Self::ERROR)
    }
}

/// One logical observation delivered by a poller.
///
/// A wait coalesces split native hints for one registration into one resource
/// event. Distinct registrations remain distinct even when their keys match.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    /// Readiness observed for a registered resource.
    #[non_exhaustive]
    Resource {
        /// Exact registration that produced the event.
        registration: Registration,
        /// Caller key supplied at registration.
        key: Key,
        /// The union of advisory readiness hints reported for the resource.
        readiness: Readiness,
    },
    /// An explicit wake observed by the poller.
    #[non_exhaustive]
    Wake {
        /// Caller key configured for the wake capability.
        key: Key,
    },
}

impl Event {
    /// Returns the caller key carried by this event.
    pub const fn key(self) -> Key {
        match self {
            Self::Resource { key, .. } | Self::Wake { key } => key,
        }
    }

    /// Returns the exact registration for a resource event.
    pub const fn registration(self) -> Option<Registration> {
        match self {
            Self::Resource { registration, .. } => Some(registration),
            Self::Wake { .. } => None,
        }
    }

    /// Returns readiness for a resource event.
    pub const fn readiness(self) -> Option<Readiness> {
        match self {
            Self::Resource { readiness, .. } => Some(readiness),
            Self::Wake { .. } => None,
        }
    }
}

#[cfg(test)]
#[path = "event_test.rs"]
mod tests;
