//! Small common adapter seam for native candidates.

use std::{os::unix::net::UnixStream, time::Duration};

use crate::{
    ConfiguredDelivery, DeliveryProfile, Observation, ProfileSupport, model::RegistrationSpec,
};

pub(crate) type CandidateResult<T> = Result<T, String>;

pub(crate) trait Candidate {
    type Session<'source>: CandidateSession
    where
        Self: 'source;

    fn support(profile: DeliveryProfile) -> CandidateResult<ProfileSupport>;

    fn configured_delivery(profile: DeliveryProfile) -> ConfiguredDelivery;

    fn register(source: &UnixStream, spec: RegistrationSpec) -> CandidateResult<Self::Session<'_>>;
}

pub(crate) trait CandidateSession {
    fn wait(&mut self, timeout: Duration) -> CandidateResult<EventBatch>;

    fn rearm(&mut self) -> CandidateResult<()>;

    fn delete(self) -> CandidateResult<()>;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct EventBatch {
    pub(crate) matched_events: usize,
    pub(crate) observation: Observation,
}

#[cfg(test)]
impl EventBatch {
    pub(crate) const fn empty() -> Self {
        Self {
            matched_events: 0,
            observation: Observation::EMPTY,
        }
    }

    pub(crate) const fn one(observation: Observation) -> Self {
        Self {
            matched_events: 1,
            observation,
        }
    }
}
