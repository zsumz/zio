//! Zio borrowed-registration adapter for common readiness workloads.
//!
//! A successful registration carries its source lifetime until controlled
//! deletion. Mutation and observation errors first drop the poller, retiring
//! every raw backend registration before a lifetime guard can disappear.

use std::{marker::PhantomData, os::unix::net::UnixStream, time::Duration};

use zio::{Event, Key, Mode, Poll, Registration, Waker};

use super::backend::{Backend, Profile, display};
use super::zio_backend::finish_wait;

pub(crate) struct ZioBorrowedBackend {
    poll: Option<Poll>,
    events: zio::Events,
    waker: Option<Waker>,
}

#[repr(transparent)]
pub(crate) struct ZioBorrowedRegistration<'source> {
    pub(super) registration: Registration,
    source: PhantomData<&'source UnixStream>,
}

impl ZioBorrowedBackend {
    #[inline]
    pub(super) fn poll_mut(&mut self) -> Result<&mut Poll, String> {
        self.poll.as_mut().ok_or_else(invalidated)
    }

    #[cold]
    #[inline(never)]
    fn invalidate(&mut self, error: impl std::fmt::Display) -> String {
        let message = error.to_string();
        self.waker = None;
        drop(self.poll.take());
        message
    }
}

impl Backend for ZioBorrowedBackend {
    type Registration<'source> = ZioBorrowedRegistration<'source>;
    type Wake = Waker;

    fn new(event_capacity: usize, registration_capacity: usize) -> Result<Self, String> {
        let poll = Poll::with_capacity(event_capacity, registration_capacity).map_err(display)?;
        let events = poll.events().map_err(display)?;
        Ok(Self {
            poll: Some(poll),
            events,
            waker: None,
        })
    }

    #[allow(
        unsafe_code,
        reason = "sources live through deletion; adapter errors terminally invalidate the poller"
    )]
    #[inline]
    fn register<'source>(
        &mut self,
        source: &'source UnixStream,
        key: usize,
        profile: Profile,
    ) -> Result<Self::Registration<'source>, String> {
        let key = Key::try_from(key).map_err(|error| self.invalidate(error))?;
        // SAFETY: every workload keeps `source` alive and unchanged through a
        // proven successful delete before it drops the candidate poller. On
        // failure, `invalidate` drops the poller before this borrow can end.
        let result = unsafe {
            self.poll_mut()?
                .register_borrowed(source, key, zio::Interest::READABLE, mode(profile))
        };
        let registration = result.map_err(|error| self.invalidate(error))?;
        Ok(ZioBorrowedRegistration {
            registration,
            source: PhantomData,
        })
    }

    fn rearm(
        &mut self,
        registration: &Self::Registration<'_>,
        profile: Profile,
    ) -> Result<(), String> {
        let result = self.poll_mut()?.modify(
            &registration.registration,
            zio::Interest::READABLE,
            mode(profile),
        );
        result.map_err(|error| self.invalidate(error))
    }

    #[inline]
    fn delete(&mut self, registration: Self::Registration<'_>) -> Result<(), String> {
        let result = self.poll_mut()?.delete(registration.registration);
        result.map_err(|error| self.invalidate(error))
    }

    fn wait(
        &mut self,
        timeout: Duration,
        observe: &mut dyn FnMut(usize) -> Result<(), String>,
    ) -> Result<usize, String> {
        let result = match self.poll.as_mut() {
            Some(poll) => poll.wait(&mut self.events, timeout.into()),
            None => return Err("zio borrowed backend was invalidated".to_owned()),
        };
        let report = result.map_err(|error| self.invalidate(error))?;
        let translated = (|| {
            let mut count = 0_usize;
            for event in &self.events {
                match *event {
                    Event::Resource { key, readiness, .. } if readiness.is_readable() => {
                        observe(usize::try_from(key).map_err(display)?)?;
                        count = count.saturating_add(1);
                    }
                    Event::Resource { key, readiness, .. } => {
                        return Err(format!(
                            "zio borrowed key {key} was not readable: {readiness:?}"
                        ));
                    }
                    Event::Wake { key, .. } => {
                        return Err(format!("unexpected zio borrowed wake key {key}"));
                    }
                }
            }
            Ok(count)
        })();
        finish_wait(translated, report).map_err(|error| self.invalidate(error))
    }

    fn configure_wake(&mut self) -> Result<(), String> {
        let result = self.poll_mut()?.waker(Key::from(u64::MAX));
        self.waker = Some(result.map_err(|error| self.invalidate(error))?);
        Ok(())
    }

    fn wake_handle(&self) -> Result<Self::Wake, String> {
        self.waker
            .clone()
            .ok_or_else(|| "zio borrowed wake was not configured".to_owned())
    }

    fn wait_for_wake(&mut self, timeout: Duration) -> Result<u64, String> {
        let result = match self.poll.as_mut() {
            Some(poll) => poll.wait(&mut self.events, timeout.into()),
            None => return Err("zio borrowed backend was invalidated".to_owned()),
        };
        let report = result.map_err(|error| self.invalidate(error))?;
        let delivery = match self.events.as_slice() {
            [Event::Wake { key, .. }] if *key == Key::from(u64::MAX) => Ok(1),
            events => Err(format!(
                "unexpected zio borrowed wake observations: {events:?}"
            )),
        };
        finish_wait(delivery, report).map_err(|error| self.invalidate(error))
    }
}

#[cold]
#[inline(never)]
fn invalidated() -> String {
    "zio borrowed backend was invalidated".to_owned()
}

const fn mode(profile: Profile) -> Mode {
    match profile {
        Profile::InitialObservation | Profile::Persistent | Profile::Level => Mode::Level,
        Profile::OneShot => Mode::OneShot,
    }
}
