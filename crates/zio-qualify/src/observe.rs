//! Delivery-profile behavior and contract validation.

use std::time::{Duration, Instant};

use crate::{
    ContractViolation, DeliveryProfile, Implementation, Observation, QualificationFailure,
    QualificationPhase, Scenario,
    candidate::{CandidateSession, EventBatch},
    contract::expectation_for,
};

const WAIT_LIMIT: Duration = Duration::from_secs(1);
const QUIET_WINDOW: Duration = Duration::from_millis(20);

pub(crate) fn verify_initial_quiet<S: CandidateSession>(
    session: &mut S,
    implementation: Implementation,
    scenario: Scenario,
) -> Result<(), QualificationFailure> {
    verify_quiet(
        session,
        implementation,
        scenario,
        QualificationPhase::Quiescence,
        "no readiness before fixture activation",
    )
}

pub(crate) fn observe_profile<S: CandidateSession>(
    session: &mut S,
    implementation: Implementation,
    scenario: Scenario,
    observations: &mut Vec<Observation>,
) -> Result<(), QualificationFailure> {
    observe_one(
        session,
        implementation,
        scenario,
        QualificationPhase::Wait,
        observations,
    )?;
    match scenario.profile() {
        DeliveryProfile::InitialObservation => Ok(()),
        DeliveryProfile::Level => observe_one(
            session,
            implementation,
            scenario,
            QualificationPhase::LevelDelivery,
            observations,
        ),
        DeliveryProfile::OneShot => {
            verify_disarmed(session, implementation, scenario, "before explicit rearm")?;
            session.rearm().map_err(|actual| {
                failure(
                    implementation,
                    scenario,
                    QualificationPhase::Rearm,
                    "explicit native rearm",
                    actual,
                )
            })?;
            observe_one(
                session,
                implementation,
                scenario,
                QualificationPhase::Rearm,
                observations,
            )?;
            verify_disarmed(session, implementation, scenario, "after rearmed delivery")
        }
    }
}

fn verify_disarmed<S: CandidateSession>(
    session: &mut S,
    implementation: Implementation,
    scenario: Scenario,
    position: &'static str,
) -> Result<(), QualificationFailure> {
    verify_quiet(
        session,
        implementation,
        scenario,
        QualificationPhase::Disarm,
        format!("no delivery {position}"),
    )
}

fn verify_quiet<S: CandidateSession>(
    session: &mut S,
    implementation: Implementation,
    scenario: Scenario,
    phase: QualificationPhase,
    expected: impl Into<String>,
) -> Result<(), QualificationFailure> {
    let expected = expected.into();
    let deadline = Instant::now() + QUIET_WINDOW;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let batch = session.wait(remaining).map_err(|actual| {
            failure(
                implementation,
                scenario,
                phase,
                format!("successful quiet-window wait: {expected}"),
                actual,
            )
        })?;
        if batch.matched_events != 0 {
            return Err(failure(
                implementation,
                scenario,
                phase,
                expected,
                batch_description(batch),
            ));
        }
        if Instant::now() >= deadline {
            return Ok(());
        }
    }
}

fn observe_one<S: CandidateSession>(
    session: &mut S,
    implementation: Implementation,
    scenario: Scenario,
    phase: QualificationPhase,
    observations: &mut Vec<Observation>,
) -> Result<(), QualificationFailure> {
    let batch = await_event(session, WAIT_LIMIT).map_err(|actual| {
        failure(
            implementation,
            scenario,
            phase,
            "successful readiness wait",
            actual,
        )
    })?;
    if batch.matched_events == 0 {
        return Err(failure(
            implementation,
            scenario,
            phase,
            "one readiness observation before deadline",
            "deadline elapsed",
        ));
    }
    let actual = batch.observation;
    observations.push(actual);
    if batch.matched_events != 1 {
        return Err(failure(
            implementation,
            scenario,
            QualificationPhase::Cardinality,
            "exactly one matching readiness event",
            batch_description(batch),
        ));
    }
    expectation_for(scenario).validate(actual).map_err(|cause| {
        failure(
            implementation,
            scenario,
            QualificationPhase::Contract,
            contract_expected(cause),
            actual.to_string(),
        )
    })
}

fn await_event<S: CandidateSession>(
    session: &mut S,
    limit: Duration,
) -> Result<EventBatch, String> {
    let deadline = Instant::now() + limit;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let batch = session.wait(remaining)?;
        if batch.matched_events != 0 || Instant::now() >= deadline {
            return Ok(batch);
        }
    }
}

fn batch_description(batch: EventBatch) -> String {
    format!(
        "{} matching events with {}",
        batch.matched_events, batch.observation
    )
}

fn contract_expected(cause: ContractViolation) -> String {
    match cause {
        ContractViolation::MissingRequired { required, .. } => {
            format!("required flags {required}")
        }
        ContractViolation::MissingOneOf { required_any, .. } => {
            format!("at least one flag from {required_any}")
        }
        ContractViolation::Undocumented { allowed, .. } => {
            format!("no flags outside {allowed}")
        }
    }
}

fn failure(
    implementation: Implementation,
    scenario: Scenario,
    phase: QualificationPhase,
    expected: impl Into<String>,
    actual: impl Into<String>,
) -> QualificationFailure {
    QualificationFailure::new(implementation, scenario, phase, expected, actual)
}
