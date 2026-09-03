//! Poll ownership, registration mutation, and wake capabilities.

use std::{fmt, num::NonZeroUsize};

use crate::{
    CapacityKind, CapacityReason, Error, Events, Key, Operation,
    registration::PollOwner,
    sys::{Backend, RawBatch, Wake},
    table::RegistrationTable,
};

/// Default delivered event capacity for one poll operation.
pub const DEFAULT_EVENT_CAPACITY: usize = 1_024;
/// Default number of registrations retained by one poller.
pub const DEFAULT_REGISTRATION_CAPACITY: usize = 1_024;

/// Cloneable cross-thread capability that interrupts its owning poller.
#[derive(Clone)]
pub struct Waker {
    wake: Wake,
    key: Key,
}

impl fmt::Debug for Waker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Waker")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl Waker {
    /// Returns the key carried by this waker's observations.
    pub const fn key(&self) -> Key {
        self.key
    }

    /// Makes the poller's configured wake key observable.
    ///
    /// Multiple triggers may coalesce into one wake event. Wake observations
    /// are notifications rather than a count of calls to this method.
    pub fn wake(&self) -> Result<(), Error> {
        self.wake.wake().map_err(|source| Error::Io {
            operation: Operation::TriggerWake,
            source,
        })
    }
}

/// Owner-local portable readiness poller.
///
/// A poller can move between threads. Operations require exclusive access.
pub struct Poll {
    pub(crate) owner: PollOwner,
    pub(crate) backend: Backend,
    pub(crate) raw_events: RawBatch,
    pub(crate) registrations: RegistrationTable,
    pub(crate) event_capacity: NonZeroUsize,
    pub(crate) wake: Wake,
    pub(crate) wake_key: Option<Key>,
    pub(crate) pending: crate::pending::PendingBatch,
}

impl fmt::Debug for Poll {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Poll")
            .field("event_capacity", &self.event_capacity())
            .field("registration_capacity", &self.registration_capacity())
            .field("registration_count", &self.registration_count())
            .field(
                "remaining_registration_capacity",
                &self.remaining_registration_capacity(),
            )
            .field("wake_key", &self.waker_key())
            .finish_non_exhaustive()
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]
impl std::os::fd::AsFd for Poll {
    /// Borrows the native selector descriptor.
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.backend.as_fd()
    }
}

impl Poll {
    /// Creates a poller with the default fixed capacities.
    pub fn new() -> Result<Self, Error> {
        Self::with_capacity(DEFAULT_EVENT_CAPACITY, DEFAULT_REGISTRATION_CAPACITY)
    }

    /// Creates a poller with fixed event and registration capacities.
    pub fn with_capacity(events: usize, registrations: usize) -> Result<Self, Error> {
        let event_capacity = NonZeroUsize::new(events).ok_or(Error::Capacity {
            kind: CapacityKind::Event,
            limit: events,
            reason: CapacityReason::Zero,
        })?;
        let registration_capacity = NonZeroUsize::new(registrations).ok_or(Error::Capacity {
            kind: CapacityKind::Registration,
            limit: registrations,
            reason: CapacityReason::Zero,
        })?;
        RegistrationTable::validate_capacity(registration_capacity)?;
        let raw_events = Backend::raw_batch(event_capacity.get(), registration_capacity.get())?;
        let registrations = RegistrationTable::new(registration_capacity)?;
        let (backend, wake) = Backend::new().map_err(|failure| {
            if failure.operation() == Operation::UnsupportedPlatform {
                Error::UnsupportedPlatform
            } else {
                Error::Io {
                    operation: failure.operation(),
                    source: failure.into_source(),
                }
            }
        })?;
        Ok(Self {
            owner: PollOwner::unassigned(),
            backend,
            raw_events,
            registrations,
            event_capacity,
            wake,
            wake_key: None,
            pending: crate::pending::PendingBatch::new(event_capacity, registration_capacity)?,
        })
    }

    /// Allocates an empty reusable destination sized for this poller.
    pub fn events(&self) -> Result<Events, Error> {
        Events::new(self.event_capacity)
    }

    /// Returns the fixed delivered-event capacity.
    pub const fn event_capacity(&self) -> usize {
        self.event_capacity.get()
    }

    /// Returns the fixed registration capacity.
    pub const fn registration_capacity(&self) -> usize {
        self.registrations.capacity()
    }

    /// Returns the retained registration count, including uncertain entries.
    pub const fn registration_count(&self) -> usize {
        self.registrations.len()
    }

    /// Returns the number of registration slots currently reservable.
    pub const fn remaining_registration_capacity(&self) -> usize {
        self.registrations.remaining()
    }

    /// Returns the configured wake key, when one is bound.
    pub const fn waker_key(&self) -> Option<Key> {
        self.wake_key
    }

    /// Returns a cloneable wake capability associated with `key`.
    ///
    /// The first successful call fixes the poller's wake key. Later calls with
    /// the same key return another capability; a different key is rejected
    /// without replacing the existing configuration.
    pub fn waker(&mut self, key: Key) -> Result<Waker, Error> {
        match self.wake_key {
            None => self.wake_key = Some(key),
            Some(existing) if existing == key => {}
            Some(existing) => {
                return Err(Error::WakerAlreadyConfigured {
                    existing,
                    requested: key,
                });
            }
        }
        Ok(Waker {
            wake: self.wake.clone(),
            key,
        })
    }
}

#[cfg(test)]
#[path = "poll_test.rs"]
mod tests;
