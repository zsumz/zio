//! Shared black-box wake construction and observation checks.

use std::fmt::{Debug, Display};

use zio::{Event, Events, Key, Poll, Wait, Waker};

use crate::{WakeCheck, WakeFailure, WakeScenario};

pub(crate) fn poll(
    scenario: WakeScenario,
    event_capacity: usize,
    registration_capacity: usize,
) -> Result<Poll, WakeFailure> {
    Poll::builder()
        .event_capacity(event_capacity)
        .registration_capacity(registration_capacity)
        .build()
        .map_err(|error| observed(scenario, WakeCheck::Setup, "constructed poller", &error))
}

pub(crate) fn events(poll: &Poll, scenario: WakeScenario) -> Result<Events, WakeFailure> {
    poll.events()
        .map_err(|error| observed(scenario, WakeCheck::Setup, "event destination", &error))
}

pub(crate) fn waker(
    poll: &mut Poll,
    key: Key,
    scenario: WakeScenario,
) -> Result<Waker, WakeFailure> {
    poll.waker(key).map_err(|error| {
        observed(
            scenario,
            WakeCheck::Configuration,
            "configured waker",
            &error,
        )
    })
}

pub(crate) fn trigger(waker: &Waker, scenario: WakeScenario) -> Result<(), WakeFailure> {
    waker
        .wake()
        .map_err(|error| observed(scenario, WakeCheck::Trigger, "successful wake", &error))
}

pub(crate) fn wait_for(
    poll: &mut Poll,
    events: &mut Events,
    wait: Wait,
    scenario: WakeScenario,
) -> Result<zio::WaitReport, WakeFailure> {
    poll.wait(events, wait)
        .map_err(|error| observed(scenario, WakeCheck::Wait, "successful wait", &error))
}

pub(crate) fn reject_recovery(
    report: zio::WaitReport,
    events: &Events,
    scenario: WakeScenario,
) -> Result<(), WakeFailure> {
    match report.into_recovery() {
        None => Ok(()),
        Some(recovery) => mismatch(
            scenario,
            WakeCheck::Recovery,
            "valid delivery without post-delivery recovery trouble",
            format!("{recovery}; delivered events: {:?}", events.as_slice()),
        ),
    }
}

pub(crate) fn expect_single_wake(
    events: &Events,
    key: Key,
    scenario: WakeScenario,
) -> Result<(), WakeFailure> {
    match events.as_slice() {
        [Event::Wake { key: actual, .. }] if *actual == key => Ok(()),
        actual => mismatch(
            scenario,
            WakeCheck::Events,
            format!("[Wake {{ key: {key:?} }}]"),
            actual,
        ),
    }
}

pub(crate) fn expect_empty(events: &Events, scenario: WakeScenario) -> Result<(), WakeFailure> {
    if events.is_empty() {
        Ok(())
    } else {
        mismatch(
            scenario,
            WakeCheck::Drain,
            "empty event destination",
            events.as_slice(),
        )
    }
}

pub(crate) fn observed(
    scenario: WakeScenario,
    check: WakeCheck,
    expected: impl Into<String>,
    actual: &impl Display,
) -> WakeFailure {
    WakeFailure::new(scenario, check, expected, actual.to_string())
}

pub(crate) fn mismatch<T>(
    scenario: WakeScenario,
    check: WakeCheck,
    expected: impl Debug,
    actual: impl Debug,
) -> Result<T, WakeFailure> {
    Err(WakeFailure::new(
        scenario,
        check,
        format!("{expected:?}"),
        format!("{actual:?}"),
    ))
}
