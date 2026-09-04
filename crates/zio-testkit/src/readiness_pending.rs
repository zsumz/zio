//! Buffered-data-then-EOF observation with split-delivery handling.

use std::{io::Read, os::fd::AsFd, time::Duration};

use zio::{ArmState, Event, Key, Mode, Readiness, Registration, RegistrationState, Wait};

use crate::readiness_expectation::{ExpectedReadiness, closure_for, expected_for};
use crate::readiness_verify::{mismatch, observed, reject_recovery};
use crate::{ReadinessCheck, ReadinessFailure, ReadinessScenario};

pub(crate) const RESOURCE_KEY: Key = Key::new(5_001);
const OBSERVATION_LIMIT: Duration = Duration::from_secs(1);

pub(crate) fn observe_pending_eof<F: AsFd + Read>(
    source: &mut F,
    payload: &[u8],
    scenario: ReadinessScenario,
) -> Result<(), ReadinessFailure> {
    let mut poll = zio::Poll::builder()
        .event_capacity(4)
        .registration_capacity(1)
        .build()
        .map_err(|error| {
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
                "registered pending-EOF resource",
                &error,
            )
        })?;
    let mut events = poll
        .events()
        .map_err(|error| observed(scenario, ReadinessCheck::Setup, "event destination", &error))?;

    let result = observe_stages(
        &mut poll,
        &mut events,
        &registration,
        source,
        payload,
        scenario,
    );
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

fn observe_stages<F: Read>(
    poll: &mut zio::Poll,
    events: &mut zio::Events,
    registration: &Registration,
    source: &mut F,
    payload: &[u8],
    scenario: ReadinessScenario,
) -> Result<(), ReadinessFailure> {
    let (initial, report) = wait_readiness(poll, events, scenario)?;
    verify_expected(initial, expected_for(scenario), scenario)?;
    reject_recovery(report, events, scenario)?;
    verify_delivery_state(poll, events, registration, scenario)?;
    read_payload(source, payload, scenario)?;

    if !initial.contains(Readiness::READ_CLOSED) {
        if scenario.mode() == Mode::OneShot {
            poll.modify(registration, scenario.interest(), scenario.mode())
                .map_err(|error| {
                    observed(
                        scenario,
                        ReadinessCheck::State,
                        "explicit one-shot rearm",
                        &error,
                    )
                })?;
            verify_arm(poll, registration, ArmState::Armed, scenario)?;
        }
        let (closure, report) = wait_readiness(poll, events, scenario)?;
        verify_expected(closure, closure_for(scenario), scenario)?;
        reject_recovery(report, events, scenario)?;
        verify_delivery_state(poll, events, registration, scenario)?;
    }
    read_eof(source, scenario)
}

pub(crate) fn wait_readiness(
    poll: &mut zio::Poll,
    events: &mut zio::Events,
    scenario: ReadinessScenario,
) -> Result<(Readiness, zio::WaitReport), ReadinessFailure> {
    let report = poll
        .wait(events, Wait::For(OBSERVATION_LIMIT))
        .map_err(|error| observed(scenario, ReadinessCheck::Wait, "successful wait", &error))?;
    match events.as_slice() {
        [Event::Resource { key, readiness, .. }] if *key == RESOURCE_KEY => {
            Ok((*readiness, report))
        }
        actual => mismatch(
            scenario,
            ReadinessCheck::Events,
            format!("one resource event with key {RESOURCE_KEY:?}"),
            actual,
        ),
    }
}

pub(crate) fn verify_expected(
    actual: Readiness,
    expected: ExpectedReadiness,
    scenario: ReadinessScenario,
) -> Result<(), ReadinessFailure> {
    if !expected.has_required(actual) {
        return mismatch(
            scenario,
            ReadinessCheck::RequiredReadiness,
            expected.required_description(),
            actual,
        );
    }
    if !expected.allowed.contains(actual) {
        return mismatch(
            scenario,
            ReadinessCheck::AllowedReadiness,
            expected.allowed,
            actual,
        );
    }
    Ok(())
}

pub(crate) fn verify_delivery_state(
    poll: &mut zio::Poll,
    events: &mut zio::Events,
    registration: &Registration,
    scenario: ReadinessScenario,
) -> Result<(), ReadinessFailure> {
    let arm = if scenario.mode() == Mode::OneShot {
        ArmState::Disarmed
    } else {
        ArmState::Armed
    };
    verify_arm(poll, registration, arm, scenario)?;
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
    Ok(())
}

pub(crate) fn verify_arm(
    poll: &zio::Poll,
    registration: &Registration,
    arm: ArmState,
    scenario: ReadinessScenario,
) -> Result<(), ReadinessFailure> {
    let expected = RegistrationState::Registered { arm };
    match poll.registration_state(registration) {
        Ok(actual) if actual == expected => Ok(()),
        actual => mismatch(scenario, ReadinessCheck::State, expected, actual),
    }
}

pub(crate) fn read_payload(
    source: &mut impl Read,
    payload: &[u8],
    scenario: ReadinessScenario,
) -> Result<(), ReadinessFailure> {
    let mut observed_payload = vec![0_u8; payload.len()];
    source.read_exact(&mut observed_payload).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Operation,
            "complete pending payload",
            &error,
        )
    })?;
    if observed_payload == payload {
        Ok(())
    } else {
        mismatch(
            scenario,
            ReadinessCheck::Operation,
            payload,
            observed_payload,
        )
    }
}

pub(crate) fn read_eof(
    source: &mut impl Read,
    scenario: ReadinessScenario,
) -> Result<(), ReadinessFailure> {
    let mut tail = [0_u8; 1];
    match source.read(&mut tail) {
        Ok(0) => Ok(()),
        actual => mismatch(
            scenario,
            ReadinessCheck::Operation,
            "zero-byte EOF after pending payload",
            actual,
        ),
    }
}
