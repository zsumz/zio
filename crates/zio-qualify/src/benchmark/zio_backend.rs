//! `zio` candidate adapter for common readiness workloads.

use std::{os::unix::net::UnixStream, time::Duration};

use zio::{Event, Key, Mode, Poll, Registration, Waker};

use super::backend::{Backend, Profile, WakeHandle, display};

pub(crate) struct ZioBackend {
    poll: Poll,
    events: zio::Events,
    waker: Option<Waker>,
}

impl Backend for ZioBackend {
    type Registration<'source> = Registration;
    type Wake = Waker;

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
        let key = Key::try_from(key).map_err(display)?;
        self.poll
            .register(source, key, zio::Interest::READABLE, mode(profile))
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
        let report = self
            .poll
            .wait(&mut self.events, timeout.into())
            .map_err(display)?;
        let delivery = (|| {
            let mut count = 0_usize;
            for event in &self.events {
                match *event {
                    Event::Resource { key, readiness, .. } if readiness.is_readable() => {
                        observe(usize::try_from(key).map_err(display)?)?;
                        count = count.saturating_add(1);
                    }
                    Event::Resource { key, readiness, .. } => {
                        return Err(format!("zio key {key} was not readable: {readiness:?}"));
                    }
                    Event::Wake { key, .. } => {
                        return Err(format!("unexpected zio wake key {key}"));
                    }
                }
            }
            Ok(count)
        })();
        finish_wait(delivery, report)
    }

    fn configure_wake(&mut self) -> Result<(), String> {
        self.waker = Some(self.poll.waker(Key::from(u64::MAX)).map_err(display)?);
        Ok(())
    }

    fn wake_handle(&self) -> Result<Self::Wake, String> {
        self.waker
            .clone()
            .ok_or_else(|| "zio wake was not configured".to_owned())
    }

    fn wait_for_wake(&mut self, timeout: Duration) -> Result<u64, String> {
        let report = self
            .poll
            .wait(&mut self.events, timeout.into())
            .map_err(display)?;
        let delivery = match self.events.as_slice() {
            [Event::Wake { key, .. }] if *key == Key::from(u64::MAX) => Ok(1),
            events => Err(format!("unexpected zio wake observations: {events:?}")),
        };
        finish_wait(delivery, report)
    }
}

pub(super) fn finish_wait<T>(
    delivery: Result<T, String>,
    report: zio::WaitReport,
) -> Result<T, String> {
    match (delivery, report.into_recovery()) {
        (Err(error), _) => Err(error),
        (Ok(_), Some(recovery)) => {
            Err(format!("unexpected zio post-delivery recovery: {recovery}"))
        }
        (Ok(delivery), None) => Ok(delivery),
    }
}

impl WakeHandle for Waker {
    fn wake(&self) -> Result<(), String> {
        self.wake().map_err(display)
    }
}

const fn mode(profile: Profile) -> Mode {
    match profile {
        Profile::InitialObservation | Profile::Persistent | Profile::Level => Mode::Level,
        Profile::OneShot => Mode::OneShot,
    }
}
