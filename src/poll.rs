//! Poll ownership, registration mutation, and wake capabilities.

use std::{
    num::NonZeroUsize,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{
    Error, Events, Key, Operation,
    registration::PollId,
    sys::{Backend, RawBatch, Wake},
    table::RegistrationTable,
};

/// Default raw event capacity for one poll operation.
pub const DEFAULT_EVENT_CAPACITY: usize = 1_024;
/// Default number of registrations retained by one poller.
pub const DEFAULT_REGISTRATION_CAPACITY: usize = 1_024;

/// Cloneable capability that interrupts its owning poller.
#[derive(Clone, Debug)]
pub struct Waker {
    wake: Wake,
}

impl Waker {
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
#[derive(Debug)]
pub struct Poll {
    pub(crate) id: PollId,
    pub(crate) backend: Backend,
    pub(crate) raw_events: RawBatch,
    pub(crate) registrations: RegistrationTable,
    pub(crate) event_capacity: NonZeroUsize,
    pub(crate) wake: Wake,
    pub(crate) wake_key: Option<Key>,
    pub(crate) pending: crate::pending::PendingBatch,
}

impl Poll {
    /// Creates a poller with the default fixed capacities.
    pub fn new() -> Result<Self, Error> {
        Self::with_capacity(DEFAULT_EVENT_CAPACITY, DEFAULT_REGISTRATION_CAPACITY)
    }

    /// Creates a poller with fixed event and registration capacities.
    pub fn with_capacity(events: usize, registrations: usize) -> Result<Self, Error> {
        let event_capacity = NonZeroUsize::new(events).ok_or(Error::Capacity { limit: events })?;
        let registration_capacity = NonZeroUsize::new(registrations).ok_or(Error::Capacity {
            limit: registrations,
        })?;
        let raw_events = Backend::raw_batch(event_capacity.get(), registration_capacity.get())
            .ok_or(Error::BackendOverflow)?;
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
            id: next_poll_id()?,
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

    /// Returns the fixed raw-event capacity.
    pub const fn event_capacity(&self) -> usize {
        self.event_capacity.get()
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
        })
    }
}

pub(crate) fn next_poll_id() -> Result<PollId, Error> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        value.checked_add(1)
    })
    .map(PollId::new)
    .map_err(|_| Error::Invariant)
}
