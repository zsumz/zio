//! Zio borrowed-registration adapter using only its public API.

use std::{os::unix::net::UnixStream, time::Duration};

use zio::{Event, Key, Mode, Poll, Readiness, Registration, Wait};

use crate::{
    ConfiguredDelivery, DeliveryProfile, Interest, Observation, ProfileSupport,
    candidate::{Candidate, CandidateResult, CandidateSession, EventBatch},
    model::RegistrationSpec,
};

pub(crate) struct ZioBorrowedCandidate;

impl Candidate for ZioBorrowedCandidate {
    type Session<'source> = ZioBorrowedSession<'source>;

    fn support(_profile: DeliveryProfile) -> CandidateResult<ProfileSupport> {
        Ok(ProfileSupport::Native)
    }

    fn configured_delivery(profile: DeliveryProfile) -> ConfiguredDelivery {
        match profile {
            DeliveryProfile::InitialObservation | DeliveryProfile::Level => {
                ConfiguredDelivery::Level
            }
            DeliveryProfile::OneShot => ConfiguredDelivery::OneShot,
        }
    }

    #[allow(
        unsafe_code,
        reason = "the session retains source until successful deletion and owns the poller"
    )]
    fn register(source: &UnixStream, spec: RegistrationSpec) -> CandidateResult<Self::Session<'_>> {
        let mut poll = Poll::with_capacity(4, 1).map_err(display)?;
        // SAFETY: the session retains `source` until successful deletion and
        // owns `poll`, so the descriptor cannot close before the poller.
        let registration = unsafe {
            poll.register_borrowed(
                source,
                Key::new(spec.key as u64),
                interest(spec.interest),
                mode(spec.profile),
            )
        }
        .map_err(display)?;
        let events = poll.events().map_err(display)?;
        Ok(ZioBorrowedSession {
            poll,
            events,
            registration,
            _source: source,
            spec,
        })
    }
}

pub(crate) struct ZioBorrowedSession<'source> {
    poll: Poll,
    events: zio::Events,
    registration: Registration,
    _source: &'source UnixStream,
    spec: RegistrationSpec,
}

impl CandidateSession for ZioBorrowedSession<'_> {
    fn wait(&mut self, timeout: Duration) -> CandidateResult<EventBatch> {
        self.events.clear();
        self.poll
            .wait(&mut self.events, Wait::For(timeout))
            .map_err(display)?;
        let mut observation = Observation::EMPTY;
        let mut matched_events = 0_usize;
        for event in &self.events {
            match *event {
                Event::Resource { key, readiness } if key == Key::new(self.spec.key as u64) => {
                    observation = observation | translate(readiness);
                    matched_events = matched_events.saturating_add(1);
                }
                _ => return Err(format!("unexpected zio borrowed event: {event:?}")),
            }
        }
        Ok(EventBatch {
            matched_events,
            observation,
        })
    }

    fn rearm(&mut self) -> CandidateResult<()> {
        self.poll
            .modify(
                &self.registration,
                interest(self.spec.interest),
                mode(self.spec.profile),
            )
            .map_err(display)
    }

    fn delete(mut self) -> CandidateResult<()> {
        self.poll.delete(self.registration).map_err(display)
    }
}

fn interest(value: Interest) -> zio::Interest {
    match value {
        Interest::Readable => zio::Interest::READABLE,
        Interest::Writable => zio::Interest::WRITABLE,
    }
}

fn mode(profile: DeliveryProfile) -> Mode {
    match profile {
        DeliveryProfile::InitialObservation | DeliveryProfile::Level => Mode::Level,
        DeliveryProfile::OneShot => Mode::OneShot,
    }
}

fn translate(readiness: Readiness) -> Observation {
    let mut observation = Observation::EMPTY;
    for (present, flag) in [
        (readiness.is_readable(), Observation::READABLE),
        (readiness.is_writable(), Observation::WRITABLE),
        (readiness.is_read_closed(), Observation::READ_CLOSED),
        (readiness.is_write_closed(), Observation::WRITE_CLOSED),
        (readiness.is_error(), Observation::ERROR),
    ] {
        if present {
            observation = observation | flag;
        }
    }
    observation
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
