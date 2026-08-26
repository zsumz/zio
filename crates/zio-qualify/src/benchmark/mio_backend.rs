//! Mio candidate adapter for common readiness workloads.

use std::{
    os::{fd::AsRawFd, unix::net::UnixStream},
    time::Duration,
};

use mio::{Events, Poll, Token, Waker, unix::SourceFd};

use super::backend::{Backend, Profile, display};

const WAKE_TOKEN: Token = Token(usize::MAX - 1);

pub(crate) struct MioRegistration {
    raw: std::os::fd::RawFd,
    key: usize,
}

pub(crate) struct MioBackend {
    poll: Poll,
    events: Events,
    waker: Option<Waker>,
}

impl Backend for MioBackend {
    type Registration<'source> = MioRegistration;

    fn new(event_capacity: usize, _registration_capacity: usize) -> Result<Self, String> {
        Ok(Self {
            poll: Poll::new().map_err(display)?,
            events: Events::with_capacity(event_capacity),
            waker: None,
        })
    }

    fn register<'source>(
        &mut self,
        source: &'source UnixStream,
        key: usize,
        _profile: Profile,
    ) -> Result<Self::Registration<'source>, String> {
        let raw = source.as_raw_fd();
        self.poll
            .registry()
            .register(&mut SourceFd(&raw), Token(key), mio::Interest::READABLE)
            .map_err(display)?;
        Ok(MioRegistration { raw, key })
    }

    fn rearm(
        &mut self,
        registration: &Self::Registration<'_>,
        _profile: Profile,
    ) -> Result<(), String> {
        self.poll
            .registry()
            .reregister(
                &mut SourceFd(&registration.raw),
                Token(registration.key),
                mio::Interest::READABLE,
            )
            .map_err(display)
    }

    fn delete(&mut self, registration: Self::Registration<'_>) -> Result<(), String> {
        self.poll
            .registry()
            .deregister(&mut SourceFd(&registration.raw))
            .map_err(display)
    }

    fn wait(
        &mut self,
        timeout: Duration,
        observe: &mut dyn FnMut(usize) -> Result<(), String>,
    ) -> Result<usize, String> {
        self.events.clear();
        self.poll
            .poll(&mut self.events, Some(timeout))
            .map_err(display)?;
        let mut count = 0_usize;
        for event in &self.events {
            if event.token() == WAKE_TOKEN {
                return Err("unexpected Mio wake observation".to_owned());
            }
            if !event.is_readable() {
                return Err(format!("Mio token {:?} was not readable", event.token()));
            }
            observe(event.token().0)?;
            count = count.saturating_add(1);
        }
        Ok(count)
    }

    fn configure_wake(&mut self) -> Result<(), String> {
        self.waker = Some(Waker::new(self.poll.registry(), WAKE_TOKEN).map_err(display)?);
        Ok(())
    }

    fn wake_roundtrip(&mut self, timeout: Duration) -> Result<u64, String> {
        self.waker
            .as_ref()
            .ok_or_else(|| "Mio wake was not configured".to_owned())?
            .wake()
            .map_err(display)?;
        self.events.clear();
        self.poll
            .poll(&mut self.events, Some(timeout))
            .map_err(display)?;
        let mut events = self.events.iter();
        let observed = events.next().map(mio::event::Event::token);
        if observed == Some(WAKE_TOKEN) && events.next().is_none() {
            Ok(1)
        } else {
            Err(format!("unexpected Mio wake observations: {observed:?}"))
        }
    }
}
