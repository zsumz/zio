//! `polling` candidate adapter for common readiness workloads.

use std::{num::NonZeroUsize, os::unix::net::UnixStream, sync::Arc, time::Duration};

use polling::{Event, Events, PollMode, Poller};

use crate::polling_registration::PollingRegistration;

use super::backend::{Backend, Profile, display};

pub(crate) struct PollingBackend {
    poller: Arc<Poller>,
    events: Events,
}

impl PollingBackend {
    pub(crate) fn supports_level() -> Result<bool, String> {
        Poller::new()
            .map(|poller| poller.supports_level())
            .map_err(display)
    }
}

impl Backend for PollingBackend {
    type Registration<'source> = PollingRegistration<'static, 'source>;

    fn new(event_capacity: usize, _registration_capacity: usize) -> Result<Self, String> {
        let capacity = NonZeroUsize::new(event_capacity)
            .ok_or_else(|| "polling event capacity must be nonzero".to_owned())?;
        Ok(Self {
            poller: Arc::new(Poller::new().map_err(display)?),
            events: Events::with_capacity(capacity),
        })
    }

    fn construct_once(event_capacity: usize, _registration_capacity: usize) -> Result<(), String> {
        let capacity = NonZeroUsize::new(event_capacity)
            .ok_or_else(|| "polling event capacity must be nonzero".to_owned())?;
        let poller = Poller::new().map_err(display)?;
        let events = Events::with_capacity(capacity);
        drop((poller, events));
        Ok(())
    }

    fn register<'source>(
        &mut self,
        source: &'source UnixStream,
        key: usize,
        profile: Profile,
    ) -> Result<Self::Registration<'source>, String> {
        PollingRegistration::shared(
            Arc::clone(&self.poller),
            source,
            Event::readable(key),
            mode(profile),
        )
        .map_err(display)
    }

    fn rearm(
        &mut self,
        registration: &Self::Registration<'_>,
        profile: Profile,
    ) -> Result<(), String> {
        registration
            .modify(Event::readable(0), mode(profile))
            .map_err(display)
    }

    fn delete(&mut self, registration: Self::Registration<'_>) -> Result<(), String> {
        registration.delete().map_err(display)
    }

    fn wait(
        &mut self,
        timeout: Duration,
        observe: &mut dyn FnMut(usize) -> Result<(), String>,
    ) -> Result<usize, String> {
        self.events.clear();
        self.poller
            .wait(&mut self.events, Some(timeout))
            .map_err(display)?;
        let mut count = 0_usize;
        for event in self.events.iter() {
            if !event.readable {
                return Err(format!("polling key {} was not readable", event.key));
            }
            observe(event.key)?;
            count = count.saturating_add(1);
        }
        Ok(count)
    }

    fn configure_wake(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn wake_roundtrip(&mut self, timeout: Duration) -> Result<u64, String> {
        self.poller.notify().map_err(display)?;
        self.events.clear();
        self.poller
            .wait(&mut self.events, Some(timeout))
            .map_err(display)?;
        if self.events.is_empty() {
            Ok(1)
        } else {
            Err(format!(
                "polling notify produced {} resource events",
                self.events.len()
            ))
        }
    }
}

const fn mode(profile: Profile) -> PollMode {
    match profile {
        Profile::InitialObservation | Profile::OneShot => PollMode::Oneshot,
        Profile::Level => PollMode::Level,
    }
}
