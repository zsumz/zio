//! Deterministically staged data delivery followed by EOF delivery.

use std::{io::Read, os::fd::AsFd};

use zio::{ArmState, Mode, Readiness};

use crate::readiness_expectation::{closure_for, expected_for};
use crate::readiness_pending::{
    RESOURCE_KEY, read_eof, read_payload, verify_arm, verify_delivery_state, verify_expected,
    wait_readiness,
};
use crate::readiness_verify::{mismatch, observed, reject_recovery};
use crate::{ReadinessCheck, ReadinessFailure, ReadinessScenario};

struct SplitObservation<'a, F, C> {
    poll: &'a mut zio::Poll,
    events: &'a mut zio::Events,
    registration: &'a zio::Registration,
    source: &'a mut F,
    payload: &'a [u8],
    scenario: ReadinessScenario,
    trigger_close: C,
}

pub(crate) fn observe_split_eof<F, C>(
    source: &mut F,
    payload: &[u8],
    scenario: ReadinessScenario,
    trigger_close: C,
) -> Result<(), ReadinessFailure>
where
    F: AsFd + Read,
    C: FnOnce() -> Result<(), ReadinessFailure>,
{
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
                "registered staged-EOF resource",
                &error,
            )
        })?;
    let mut events = poll
        .events()
        .map_err(|error| observed(scenario, ReadinessCheck::Setup, "event destination", &error))?;

    let result = observe_stages(SplitObservation {
        poll: &mut poll,
        events: &mut events,
        registration: &registration,
        source,
        payload,
        scenario,
        trigger_close,
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

fn observe_stages<F, C>(observation: SplitObservation<'_, F, C>) -> Result<(), ReadinessFailure>
where
    F: Read,
    C: FnOnce() -> Result<(), ReadinessFailure>,
{
    let SplitObservation {
        poll,
        events,
        registration,
        source,
        payload,
        scenario,
        trigger_close,
    } = observation;
    let (initial, report) = wait_readiness(poll, events, scenario)?;
    verify_expected(initial, expected_for(scenario), scenario)?;
    if initial.contains(Readiness::READ_CLOSED) {
        return mismatch(
            scenario,
            ReadinessCheck::Events,
            "data readiness before the staged peer close",
            initial,
        );
    }
    reject_recovery(report, events, scenario)?;
    verify_delivery_state(poll, events, registration, scenario)?;
    read_payload(source, payload, scenario)?;

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
    trigger_close()?;

    let (closure, report) = wait_readiness(poll, events, scenario)?;
    verify_expected(closure, closure_for(scenario), scenario)?;
    reject_recovery(report, events, scenario)?;
    verify_delivery_state(poll, events, registration, scenario)?;
    read_eof(source, scenario)
}
