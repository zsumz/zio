//! `zio` borrowed-registration adapter using only its public API.

use std::{os::unix::net::UnixStream, time::Duration};

use zio::{Event, Key, Mode, Poll, Registration};

use crate::{
    ConfiguredDelivery, DeliveryProfile, Interest, Observation, ProfileSupport,
    candidate::{Candidate, CandidateResult, CandidateSession, EventBatch},
    model::RegistrationSpec,
    zio_candidate::finish_wait,
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
        let mut poll = Poll::builder()
            .event_capacity(4)
            .registration_capacity(1)
            .build()
            .map_err(display)?;
        // SAFETY: the session retains `source` until successful deletion and
        // owns `poll`, so the descriptor cannot close before the poller.
        let registration = unsafe {
            poll.register_borrowed(
                source,
                Key::try_from(spec.key).map_err(display)?,
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
        let report = self
            .poll
            .wait(&mut self.events, timeout.into())
            .map_err(display)?;
        let delivery = (|| {
            let mut observation = Observation::EMPTY;
            let mut matched_events = 0_usize;
            let expected = Key::try_from(self.spec.key).map_err(display)?;
            for event in &self.events {
                if !event.is_resource() || event.key() != expected {
                    return Err(format!("unexpected zio borrowed event: {event:?}"));
                }
                observation = observation | translate(*event);
                matched_events = matched_events.saturating_add(1);
            }
            Ok(EventBatch {
                matched_events,
                observation,
            })
        })();
        finish_wait(delivery, report)
    }

    fn rearm(&mut self) -> CandidateResult<()> {
        self.poll.rearm(&self.registration).map_err(display)
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

fn translate(event: Event) -> Observation {
    let mut observation = Observation::EMPTY;
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
    observation
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
