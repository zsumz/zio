//! Shared fixture and readiness workload orchestration.

use std::{os::unix::net::UnixStream, time::Duration};

use super::{
    backend::{Backend, Profile},
    measure::{Captured, Metric, capture},
    profile_workload::{level_repeat, one_shot_disarm, one_shot_rearm},
    ready_workload::ready_transaction,
    scenario::{Scenario, WAIT_TIMEOUT_MS},
};

pub(crate) fn run<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    match scenario {
        Scenario::ConstructDrop => construct::<B>(scenario, iterations, metric),
        Scenario::RegisterDelete => register_delete::<B>(scenario, iterations, metric),
        Scenario::EmptyWait => empty_wait::<B>(scenario, iterations, metric),
        Scenario::ReadySingle | Scenario::ReadyBatch64 | Scenario::ReadyBatch1024 => {
            ready_transaction::<B>(scenario, iterations, metric)
        }
        Scenario::WakeRoundtrip => wake::<B>(scenario, iterations, metric),
        Scenario::LevelRepeat => level_repeat::<B>(scenario, iterations, metric),
        Scenario::OneShotDisarm => one_shot_disarm::<B>(scenario, iterations, metric),
        Scenario::OneShotRearm => one_shot_rearm::<B>(scenario, iterations, metric),
    }
}

fn construct<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    capture(iterations, metric, || {
        B::construct_once(scenario.event_capacity(), scenario.registration_capacity())?;
        Ok(0)
    })
}

fn register_delete<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let (source, _peer) = UnixStream::pair().map_err(display)?;
    let mut backend = B::new(scenario.event_capacity(), scenario.registration_capacity())?;
    capture(iterations, metric, || {
        let registration = backend.register(&source, 0, Profile::InitialObservation)?;
        backend.delete(registration)?;
        Ok(0)
    })
}

fn empty_wait<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let mut backend = B::new(scenario.event_capacity(), scenario.registration_capacity())?;
    capture(iterations, metric, || {
        let observed = backend.wait(Duration::ZERO, &mut |_| {
            Err("empty wait produced a resource event".to_owned())
        })?;
        if observed == 0 {
            Ok(0)
        } else {
            Err(format!("empty wait returned {observed} events"))
        }
    })
}

fn wake<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let mut backend = B::new(scenario.event_capacity(), scenario.registration_capacity())?;
    backend.configure_wake()?;
    capture(iterations, metric, || {
        backend.wake_roundtrip(Duration::from_millis(WAIT_TIMEOUT_MS))
    })
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
