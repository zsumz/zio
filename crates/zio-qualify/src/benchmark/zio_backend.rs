//! Zio candidate adapter for common readiness workloads.

use std::{os::unix::net::UnixStream, time::Duration};

use zio::{Event, Key, Mode, Poll, Registration, Wait, Waker};

use super::backend::{Backend, Profile, display};

pub(crate) struct ZioBackend {
    poll: Poll,
    events: zio::Events,
    waker: Option<Waker>,
}

impl Backend for ZioBackend {
    type Registration<'source> = Registration;

    fn new(event_capacity: usize, registration_capacity: usize) -> Result<Self, String> {
        let poll = Poll::with_capacity(event_capacity, registration_capacity).map_err(display)?;
        let events = poll.events().map_err(display)?;
        Ok(Self {
            poll,
            events,
            waker: None,
        })
    }

    fn register<'source>(
        &mut self,
        source: &'source UnixStream,
        key: usize,
        profile: Profile,
    ) -> Result<Self::Registration<'source>, String> {
        let key = u64::try_from(key).map_err(display)?;
        self.poll
            .register(
                source,
                Key::new(key),
                zio::Interest::READABLE,
                mode(profile),
            )
            .map_err(display)
    }

    fn rearm(
        &mut self,
        registration: &Self::Registration<'_>,
        profile: Profile,
    ) -> Result<(), String> {
        self.poll
            .modify(registration, zio::Interest::READABLE, mode(profile))
            .map_err(display)
    }

    fn delete(&mut self, registration: Self::Registration<'_>) -> Result<(), String> {
        self.poll.delete(registration).map_err(display)
    }

    fn wait(
        &mut self,
        timeout: Duration,
        observe: &mut dyn FnMut(usize) -> Result<(), String>,
    ) -> Result<usize, String> {
        self.events.clear();
        self.poll
            .wait(&mut self.events, Wait::For(timeout))
            .map_err(display)?;
        let mut count = 0_usize;
        for event in &self.events {
            match *event {
                Event::Resource { key, readiness } if readiness.is_readable() => {
                    observe(usize::try_from(key.get()).map_err(display)?)?;
                    count = count.saturating_add(1);
                }
                Event::Resource { key, readiness } => {
                    return Err(format!(
                        "zio key {} was not readable: {readiness:?}",
                        key.get()
                    ));
                }
                Event::Wake { key } => {
                    return Err(format!("unexpected zio wake key {}", key.get()));
                }
            }
        }
        Ok(count)
    }

    fn configure_wake(&mut self) -> Result<(), String> {
        self.waker = Some(self.poll.waker(Key::new(u64::MAX)).map_err(display)?);
        Ok(())
    }

    fn wake_roundtrip(&mut self, timeout: Duration) -> Result<u64, String> {
        let waker = self
            .waker
            .as_ref()
            .ok_or_else(|| "zio wake was not configured".to_owned())?;
        waker.wake().map_err(display)?;
        self.events.clear();
        self.poll
            .wait(&mut self.events, Wait::For(timeout))
            .map_err(display)?;
        match self.events.as_slice() {
            [Event::Wake { key }] if *key == Key::new(u64::MAX) => Ok(1),
            events => Err(format!("unexpected zio wake observations: {events:?}")),
        }
    }
}

const fn mode(profile: Profile) -> Mode {
    match profile {
        Profile::InitialObservation | Profile::Level => Mode::Level,
        Profile::OneShot => Mode::OneShot,
    }
}
