//! Adversarial adapter proofs for the qualification runner.

use std::{io, marker::PhantomData, os::unix::net::UnixStream, thread, time::Duration};

use crate::{
    CaseOutcome, ConfiguredDelivery, DeliveryProfile, Implementation, Observation, ProfileSupport,
    QualificationPhase, Scenario,
    candidate::{Candidate, CandidateResult, CandidateSession, EventBatch},
    model::RegistrationSpec,
    runner::run,
};

const ALWAYS_READY: u8 = 1;
const DUPLICATE_AND_CLEANUP: u8 = 2;
const EMPTY: u8 = 3;
const UNEXPECTED_KEY: u8 = 4;
const CLEANUP: u8 = 5;
const DISARM_LEAK: u8 = 6;
const REARM_SILENT: u8 = 7;
const WRONG_REARMED_OBSERVATION: u8 = 8;

struct FakeCandidate<const SCRIPT: u8>;

impl<const SCRIPT: u8> Candidate for FakeCandidate<SCRIPT> {
    type Session<'source> = FakeSession<'source, SCRIPT>;

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

    fn register(
        _source: &UnixStream,
        _spec: RegistrationSpec,
    ) -> CandidateResult<Self::Session<'_>> {
        Ok(FakeSession {
            rearmed: false,
            waits: 0,
            source: PhantomData,
        })
    }
}

struct FakeSession<'source, const SCRIPT: u8> {
    rearmed: bool,
    waits: usize,
    source: PhantomData<&'source UnixStream>,
}

impl<const SCRIPT: u8> CandidateSession for FakeSession<'_, SCRIPT> {
    fn wait(&mut self, timeout: Duration) -> CandidateResult<EventBatch> {
        self.waits = self.waits.saturating_add(1);
        if SCRIPT == ALWAYS_READY {
            return Ok(EventBatch::one(Observation::READABLE));
        }
        if self.waits == 1 {
            thread::sleep(timeout);
            return Ok(EventBatch::empty());
        }
        if matches!(
            SCRIPT,
            DISARM_LEAK | REARM_SILENT | WRONG_REARMED_OBSERVATION
        ) {
            if self.waits == 2 {
                return Ok(EventBatch::one(Observation::READABLE));
            }
            if SCRIPT == DISARM_LEAK {
                return Ok(EventBatch::one(Observation::READABLE));
            }
            if !self.rearmed || SCRIPT == REARM_SILENT {
                thread::sleep(timeout);
                return Ok(EventBatch::empty());
            }
            return Ok(EventBatch::one(Observation::WRITABLE));
        }
        match SCRIPT {
            DUPLICATE_AND_CLEANUP => Ok(EventBatch {
                matched_events: 2,
                observation: Observation::READABLE,
            }),
            EMPTY => {
                thread::sleep(timeout);
                Ok(EventBatch::empty())
            }
            UNEXPECTED_KEY => Err("unexpected key 99".to_owned()),
            CLEANUP => Ok(EventBatch::one(Observation::READABLE)),
            _ => Err("unknown fake script".to_owned()),
        }
    }

    fn rearm(&mut self) -> CandidateResult<()> {
        self.rearmed = true;
        Ok(())
    }

    fn delete(self) -> CandidateResult<()> {
        if matches!(SCRIPT, DUPLICATE_AND_CLEANUP | CLEANUP) {
            Err("injected cleanup failure".to_owned())
        } else {
            Ok(())
        }
    }
}

#[test]
fn always_ready_adapter_is_rejected_before_activation() -> Result<(), io::Error> {
    assert_failed_phase::<ALWAYS_READY>(QualificationPhase::Quiescence)
}

#[test]
fn duplicate_batch_and_cleanup_failures_are_both_retained() -> Result<(), io::Error> {
    let result = qualify::<DUPLICATE_AND_CLEANUP>();
    check(
        has_phase(&result, QualificationPhase::Cardinality),
        "duplicate matching events were accepted",
    )?;
    check(
        has_phase(&result, QualificationPhase::Cleanup),
        "cleanup failure was discarded after an earlier failure",
    )?;
    check(
        result.failures().len() == 2,
        "qualification did not retain both independent failures",
    )?;
    check(
        result.observations() == [Observation::READABLE],
        "translated duplicate-batch observation was not retained",
    )
}

#[test]
fn empty_adapter_is_rejected_at_wait() -> Result<(), io::Error> {
    assert_failed_phase::<EMPTY>(QualificationPhase::Wait)
}

#[test]
fn unexpected_key_adapter_is_rejected_at_wait() -> Result<(), io::Error> {
    assert_failed_phase::<UNEXPECTED_KEY>(QualificationPhase::Wait)
}

#[test]
fn cleanup_only_failure_is_rejected() -> Result<(), io::Error> {
    assert_failed_phase::<CLEANUP>(QualificationPhase::Cleanup)
}

#[test]
fn one_shot_redelivery_while_disarmed_is_rejected() -> Result<(), io::Error> {
    let result = qualify_case::<DISARM_LEAK>(Scenario::UnixReadableOneShot);
    check(
        has_phase(&result, QualificationPhase::Disarm),
        "one-shot adapter redelivered before rearm",
    )?;
    check(
        result.observations() == [Observation::READABLE],
        "first observation was not retained before disarm failure",
    )
}

#[test]
fn one_shot_rearm_without_redelivery_is_rejected() -> Result<(), io::Error> {
    let result = qualify_case::<REARM_SILENT>(Scenario::UnixReadableOneShot);
    check(
        has_phase(&result, QualificationPhase::Rearm),
        "one-shot adapter passed without rearmed delivery",
    )?;
    check(
        result.observations() == [Observation::READABLE],
        "rearm failure corrupted the first observation",
    )
}

#[test]
fn one_shot_rearmed_observation_must_match_the_contract() -> Result<(), io::Error> {
    let result = qualify_case::<WRONG_REARMED_OBSERVATION>(Scenario::UnixReadableOneShot);
    check(
        has_phase(&result, QualificationPhase::Contract),
        "wrong rearmed observation passed the contract",
    )?;
    check(
        result.observations() == [Observation::READABLE, Observation::WRITABLE],
        "rearmed observation was not retained exactly",
    )
}

fn assert_failed_phase<const SCRIPT: u8>(phase: QualificationPhase) -> Result<(), io::Error> {
    let result = qualify::<SCRIPT>();
    check(
        matches!(result.outcome(), CaseOutcome::Failed(_)),
        "adversarial adapter unexpectedly passed",
    )?;
    check(
        has_phase(&result, phase),
        "expected failure phase was absent",
    )
}

fn qualify<const SCRIPT: u8>() -> crate::CaseResult {
    qualify_case::<SCRIPT>(Scenario::UnixReadableInitial)
}

fn qualify_case<const SCRIPT: u8>(scenario: Scenario) -> crate::CaseResult {
    run::<FakeCandidate<SCRIPT>>(Implementation::Zio, scenario)
}

fn has_phase(result: &crate::CaseResult, phase: QualificationPhase) -> bool {
    result
        .failures()
        .iter()
        .any(|failure| failure.phase() == phase)
}

fn check(condition: bool, message: &'static str) -> Result<(), io::Error> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message))
    }
}
