//! Shared public-API readiness observation checks.

use std::{fmt::Debug, os::fd::AsFd, time::Duration};

use zio::{ArmState, Event, Key, Mode, Readiness, Registration, RegistrationState, Wait};

use crate::readiness_expectation::ExpectedReadiness;
use crate::{ReadinessCheck, ReadinessFailure, ReadinessScenario};

const RESOURCE_KEY: Key = Key::new(5_001);
const OBSERVATION_LIMIT: Duration = Duration::from_secs(1);

struct Observation<'a> {
    poll: &'a mut zio::Poll,
    events: &'a mut zio::Events,
    registration: &'a Registration,
    scenario: ReadinessScenario,
    expected: ExpectedReadiness,
}

pub(crate) fn observe<F, V>(
    source: &mut F,
    scenario: ReadinessScenario,
    expected: ExpectedReadiness,
    verify_operation: V,
) -> Result<(), ReadinessFailure>
where
    F: AsFd + ?Sized,
    V: FnOnce(&mut F) -> Result<(), ReadinessFailure>,
{
    observe_after_register(source, scenario, expected, || Ok(()), verify_operation)
}

pub(crate) fn observe_after_register<F, A, V>(
    source: &mut F,
    scenario: ReadinessScenario,
    expected: ExpectedReadiness,
    activate: A,
    verify_operation: V,
) -> Result<(), ReadinessFailure>
where
    F: AsFd + ?Sized,
    A: FnOnce() -> Result<(), ReadinessFailure>,
    V: FnOnce(&mut F) -> Result<(), ReadinessFailure>,
{
    let mut poll = zio::Poll::with_capacity(4, 1).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "constructed poller",
            &error,
        )
    })?;
    let registration = poll
        .register(source, RESOURCE_KEY, scenario.interest(), scenario.mode())
        .map_err(|error| {
            observed(
                scenario,
                ReadinessCheck::Setup,
                "registered native resource",
                &error,
            )
        })?;
    let mut events = poll
        .events()
        .map_err(|error| observed(scenario, ReadinessCheck::Setup, "event destination", &error))?;

    let result = activate().and_then(|()| {
        observe_registered(
            Observation {
                poll: &mut poll,
                events: &mut events,
                registration: &registration,
                scenario,
                expected,
            },
            source,
            verify_operation,
        )
    });
    let cleanup = poll.delete(registration).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Cleanup,
            "deleted registration",
            &error,
        )
    });
    match (result, cleanup) {
        (Err(failure), _) | (Ok(()), Err(failure)) => Err(failure),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn observe_registered<F, V>(
    observation: Observation<'_>,
    source: &mut F,
    verify_operation: V,
) -> Result<(), ReadinessFailure>
where
    F: ?Sized,
    V: FnOnce(&mut F) -> Result<(), ReadinessFailure>,
{
    let Observation {
        poll,
        events,
        registration,
        scenario,
        expected,
    } = observation;
    let report = poll
        .wait(events, Wait::For(OBSERVATION_LIMIT))
        .map_err(|error| observed(scenario, ReadinessCheck::Wait, "successful wait", &error))?;
    let readiness = single_readiness(events, scenario)?;
    if !expected.has_required(readiness) {
        return mismatch(
            scenario,
            ReadinessCheck::RequiredReadiness,
            expected.required_description(),
            readiness,
        );
    }
    if !expected.allowed.contains(readiness) {
        return mismatch(
            scenario,
            ReadinessCheck::AllowedReadiness,
            expected.allowed,
            readiness,
        );
    }
    reject_recovery(report, events, scenario)?;
    verify_state(poll, registration, scenario)?;
    if scenario.mode() == Mode::OneShot {
        let report = poll.wait(events, Wait::NoBlock).map_err(|error| {
            observed(
                scenario,
                ReadinessCheck::Wait,
                "successful one-shot drain",
                &error,
            )
        })?;
        if !events.is_empty() {
            return mismatch(
                scenario,
                ReadinessCheck::Disarm,
                "no delivery before explicit rearm",
                events.as_slice(),
            );
        }
        reject_recovery(report, events, scenario)?;
    }
    verify_operation(source)
}

pub(crate) fn reject_recovery(
    report: zio::WaitReport,
    events: &zio::Events,
    scenario: ReadinessScenario,
) -> Result<(), ReadinessFailure> {
    match report.into_recovery() {
        None => Ok(()),
        Some(recovery) => mismatch(
            scenario,
            ReadinessCheck::Recovery,
            "valid delivery without post-delivery recovery trouble",
            format!("{recovery}; delivered events: {:?}", events.as_slice()),
        ),
    }
}

fn single_readiness(
    events: &zio::Events,
    scenario: ReadinessScenario,
) -> Result<Readiness, ReadinessFailure> {
    match events.as_slice() {
        [Event::Resource { key, readiness }] if *key == RESOURCE_KEY => Ok(*readiness),
        actual => mismatch(
            scenario,
            ReadinessCheck::Events,
            format!("one resource event with key {RESOURCE_KEY:?}"),
            actual,
        ),
    }
}

fn verify_state(
    poll: &zio::Poll,
    registration: &Registration,
    scenario: ReadinessScenario,
) -> Result<(), ReadinessFailure> {
    let arm = if scenario.mode() == Mode::OneShot {
        ArmState::Disarmed
    } else {
        ArmState::Armed
    };
    let expected = RegistrationState::Registered { arm };
    match poll.registration_state(registration) {
        Ok(actual) if actual == expected => Ok(()),
        actual => mismatch(scenario, ReadinessCheck::State, expected, actual),
    }
}

pub(crate) fn observed(
    scenario: ReadinessScenario,
    check: ReadinessCheck,
    expected: impl Into<String>,
    actual: &impl std::fmt::Display,
) -> ReadinessFailure {
    ReadinessFailure::new(scenario, check, expected, actual.to_string())
}

pub(crate) fn mismatch<T>(
    scenario: ReadinessScenario,
    check: ReadinessCheck,
    expected: impl Debug,
    actual: impl Debug,
) -> Result<T, ReadinessFailure> {
    Err(ReadinessFailure::new(
        scenario,
        check,
        format!("{expected:?}"),
        format!("{actual:?}"),
    ))
}
