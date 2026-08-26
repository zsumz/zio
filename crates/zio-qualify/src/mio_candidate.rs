//! Mio comparator adapter.

use std::{
    os::{fd::AsRawFd, unix::net::UnixStream},
    time::Duration,
};

use mio::{Events, Poll, Token, unix::SourceFd};

use crate::{
    ConfiguredDelivery, DeliveryProfile, Interest, Observation, ProfileSupport,
    candidate::{Candidate, CandidateResult, CandidateSession, EventBatch},
    model::RegistrationSpec,
};

const MODE_REASON: &str = "Mio exposes its native default, not Level or OneShot selection";

pub(crate) struct MioCandidate;

impl Candidate for MioCandidate {
    type Session<'source> = MioSession<'source>;

    fn support(profile: DeliveryProfile) -> CandidateResult<ProfileSupport> {
        Ok(match profile {
            DeliveryProfile::InitialObservation => ProfileSupport::Native,
            DeliveryProfile::Level | DeliveryProfile::OneShot => ProfileSupport::NotExposed {
                reason: MODE_REASON,
            },
        })
    }

    fn configured_delivery(_profile: DeliveryProfile) -> ConfiguredDelivery {
        ConfiguredDelivery::NativeDefault
    }

    fn register(source: &UnixStream, spec: RegistrationSpec) -> CandidateResult<Self::Session<'_>> {
        let poll = Poll::new().map_err(display)?;
        let raw = source.as_raw_fd();
        poll.registry()
            .register(
                &mut SourceFd(&raw),
                Token(spec.key),
                interest(spec.interest),
            )
            .map_err(display)?;
        Ok(MioSession {
            poll,
            events: Events::with_capacity(4),
            source,
            spec,
        })
    }
}

pub(crate) struct MioSession<'source> {
    poll: Poll,
    events: Events,
    source: &'source UnixStream,
    spec: RegistrationSpec,
}

impl CandidateSession for MioSession<'_> {
    fn wait(&mut self, timeout: Duration) -> CandidateResult<EventBatch> {
        self.events.clear();
        self.poll
            .poll(&mut self.events, Some(timeout))
            .map_err(display)?;
        let mut observation = Observation::EMPTY;
        let mut matched_events = 0_usize;
        for event in &self.events {
            if event.token() != Token(self.spec.key) {
                return Err(format!("unexpected Mio token: {:?}", event.token()));
            }
            matched_events = matched_events.saturating_add(1);
            for (present, flag) in [
                (event.is_readable(), Observation::READABLE),
                (event.is_writable(), Observation::WRITABLE),
                (event.is_read_closed(), Observation::READ_CLOSED),
                (event.is_write_closed(), Observation::WRITE_CLOSED),
                (event.is_error(), Observation::ERROR),
            ] {
                if present {
                    observation = observation | flag;
                }
            }
        }
        Ok(EventBatch {
            matched_events,
            observation,
        })
    }

    fn rearm(&mut self) -> CandidateResult<()> {
        Err(MODE_REASON.to_owned())
    }

    fn delete(self) -> CandidateResult<()> {
        let raw = self.source.as_raw_fd();
        self.poll
            .registry()
            .deregister(&mut SourceFd(&raw))
            .map_err(display)
    }
}

fn interest(value: Interest) -> mio::Interest {
    match value {
        Interest::Readable => mio::Interest::READABLE,
        Interest::Writable => mio::Interest::WRITABLE,
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
