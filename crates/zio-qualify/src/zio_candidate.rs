//! `zio` adapter using only its ordinary public API.

use std::{os::unix::net::UnixStream, time::Duration};

use zio::{Event, Key, Mode, Poll, Registration};

use crate::{
    ConfiguredDelivery, DeliveryProfile, Interest, Observation, ProfileSupport,
    candidate::{Candidate, CandidateResult, CandidateSession, EventBatch},
    model::RegistrationSpec,
};

pub(crate) struct ZioCandidate;

impl Candidate for ZioCandidate {
    type Session<'source> = ZioSession<'source>;

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

    fn register(source: &UnixStream, spec: RegistrationSpec) -> CandidateResult<Self::Session<'_>> {
        let mut poll = Poll::with_capacity(4, 1).map_err(display)?;
        let registration = poll
            .register(
                source,
                Key::try_from(spec.key).map_err(display)?,
                interest(spec.interest),
                mode(spec.profile),
            )
            .map_err(display)?;
        let events = poll.events().map_err(display)?;
        Ok(ZioSession {
            poll,
            events,
            registration,
            _source: source,
            spec,
        })
    }
}

pub(crate) struct ZioSession<'source> {
    poll: Poll,
    events: zio::Events,
    registration: Registration,
    _source: &'source UnixStream,
    spec: RegistrationSpec,
}

impl CandidateSession for ZioSession<'_> {
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
                    return Err(format!("unexpected zio event: {event:?}"));
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

pub(crate) fn finish_wait<T>(
    delivery: CandidateResult<T>,
    report: zio::WaitReport,
) -> CandidateResult<T> {
    match (delivery, report.into_recovery()) {
        (Err(error), _) => Err(error),
        (Ok(_), Some(recovery)) => {
            Err(format!("unexpected zio post-delivery recovery: {recovery}"))
        }
        (Ok(delivery), None) => Ok(delivery),
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
