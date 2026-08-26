//! `polling` comparator adapter.

use std::{num::NonZeroUsize, os::unix::net::UnixStream, sync::Arc, time::Duration};

use polling::{Event, Events, PollMode, Poller};

use crate::{
    ConfiguredDelivery, DeliveryProfile, Interest, Observation, ProfileSupport,
    candidate::{Candidate, CandidateResult, CandidateSession, EventBatch},
    model::RegistrationSpec,
    polling_registration::PollingRegistration,
};

const LEVEL_REASON: &str = "the polling host backend reports no native Level support";
const EVENT_CAPACITY: usize = 4;

pub(crate) struct PollingCandidate;

impl Candidate for PollingCandidate {
    type Session<'source> = PollingSession<'source>;

    fn support(profile: DeliveryProfile) -> CandidateResult<ProfileSupport> {
        if profile == DeliveryProfile::Level {
            let poller = Poller::new().map_err(display)?;
            if !poller.supports_level() {
                return Ok(ProfileSupport::HostUnavailable {
                    reason: LEVEL_REASON,
                });
            }
        }
        Ok(ProfileSupport::Native)
    }

    fn configured_delivery(profile: DeliveryProfile) -> ConfiguredDelivery {
        match profile {
            DeliveryProfile::InitialObservation | DeliveryProfile::OneShot => {
                ConfiguredDelivery::OneShot
            }
            DeliveryProfile::Level => ConfiguredDelivery::Level,
        }
    }

    fn register(source: &UnixStream, spec: RegistrationSpec) -> CandidateResult<Self::Session<'_>> {
        let poller = Arc::new(Poller::new().map_err(display)?);
        let registration = event(spec);
        let mode = mode(spec.profile);
        let registration =
            PollingRegistration::shared(poller, source, registration, mode).map_err(display)?;
        let capacity = NonZeroUsize::new(EVENT_CAPACITY)
            .ok_or_else(|| "polling event capacity must be nonzero".to_owned())?;
        Ok(PollingSession {
            events: Events::with_capacity(capacity),
            registration,
            spec,
        })
    }
}

pub(crate) struct PollingSession<'source> {
    events: Events,
    registration: PollingRegistration<'static, 'source>,
    spec: RegistrationSpec,
}

impl CandidateSession for PollingSession<'_> {
    fn wait(&mut self, timeout: Duration) -> CandidateResult<EventBatch> {
        self.events.clear();
        self.registration
            .poller()
            .wait(&mut self.events, Some(timeout))
            .map_err(display)?;
        let mut observation = Observation::EMPTY;
        let mut matched_events = 0_usize;
        for ready in self.events.iter() {
            if ready.key != self.spec.key {
                return Err(format!("unexpected polling key: {}", ready.key));
            }
            matched_events = matched_events.saturating_add(1);
            for (present, flag) in [
                (ready.readable, Observation::READABLE),
                (ready.writable, Observation::WRITABLE),
                (ready.is_interrupt(), Observation::INTERRUPT),
                (ready.is_err() == Some(true), Observation::ERROR),
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
        self.registration
            .modify(event(self.spec), mode(self.spec.profile))
            .map_err(display)
    }

    fn delete(self) -> CandidateResult<()> {
        self.registration.delete().map_err(display)
    }
}

fn event(spec: RegistrationSpec) -> Event {
    match spec.interest {
        Interest::Readable => Event::readable(spec.key),
        Interest::Writable => Event::writable(spec.key),
    }
}

fn mode(profile: DeliveryProfile) -> PollMode {
    match profile {
        DeliveryProfile::InitialObservation | DeliveryProfile::OneShot => PollMode::Oneshot,
        DeliveryProfile::Level => PollMode::Level,
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
