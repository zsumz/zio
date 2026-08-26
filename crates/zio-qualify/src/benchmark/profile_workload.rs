//! Repeated level and one-shot delivery workloads.

use std::{io::Write, os::unix::net::UnixStream, time::Duration};

use super::{
    backend::{Backend, Profile},
    measure::{Captured, Metric, capture},
    scenario::{ABSENCE_WINDOW_MS, Scenario, WAIT_TIMEOUT_MS},
};

const ABSENCE_WINDOW: Duration = Duration::from_millis(ABSENCE_WINDOW_MS);

pub(crate) fn level_repeat<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let (source, mut peer) = UnixStream::pair().map_err(display)?;
    let mut backend = B::new(scenario.event_capacity(), scenario.registration_capacity())?;
    let registration = backend.register(&source, 0, Profile::Level)?;
    peer.write_all(&[1]).map_err(display)?;
    let result = capture(iterations, metric, || observe_one(&mut backend));
    let cleanup = backend.delete(registration);
    prefer_result(result, cleanup)
}

pub(crate) fn one_shot_disarm<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let (source, mut peer) = UnixStream::pair().map_err(display)?;
    let mut backend = B::new(scenario.event_capacity(), scenario.registration_capacity())?;
    let registration = backend.register(&source, 0, Profile::OneShot)?;
    peer.write_all(&[1]).map_err(display)?;
    observe_one(&mut backend)?;
    let result = capture(iterations, metric, || {
        backend.rearm(&registration, Profile::OneShot)?;
        observe_one(&mut backend)?;
        observe_absent(&mut backend)?;
        Ok(1)
    });
    let cleanup = backend.delete(registration);
    prefer_result(result, cleanup)
}

pub(crate) fn one_shot_rearm<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let (source, mut peer) = UnixStream::pair().map_err(display)?;
    let mut backend = B::new(scenario.event_capacity(), scenario.registration_capacity())?;
    let registration = backend.register(&source, 0, Profile::OneShot)?;
    peer.write_all(&[1]).map_err(display)?;
    observe_one(&mut backend)?;
    let result = capture(iterations, metric, || {
        backend.rearm(&registration, Profile::OneShot)?;
        observe_one(&mut backend)
    })
    .and_then(|captured| {
        observe_absent(&mut backend)?;
        Ok(captured)
    });
    let cleanup = backend.delete(registration);
    prefer_result(result, cleanup)
}

fn observe_one<B: Backend>(backend: &mut B) -> Result<u64, String> {
    let observed = backend.wait(Duration::from_millis(WAIT_TIMEOUT_MS), &mut |key| {
        if key == 0 {
            Ok(())
        } else {
            Err(format!("unexpected readiness key {key}"))
        }
    })?;
    if observed == 1 {
        Ok(1)
    } else {
        Err(format!("expected one readiness event, observed {observed}"))
    }
}

fn observe_absent<B: Backend>(backend: &mut B) -> Result<(), String> {
    let observed = backend.wait(ABSENCE_WINDOW, &mut |key| {
        Err(format!("one-shot emitted disarmed key {key}"))
    })?;
    if observed == 0 {
        Ok(())
    } else {
        Err(format!("one-shot absence probe returned {observed} events"))
    }
}

fn prefer_result(
    result: Result<Captured, String>,
    cleanup: Result<(), String>,
) -> Result<Captured, String> {
    match (result, cleanup) {
        (Ok(captured), Ok(())) => Ok(captured),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
