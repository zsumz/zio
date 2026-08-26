//! Shared fixture and readiness workload orchestration.

use std::{os::unix::net::UnixStream, time::Duration};

use super::{
    backend::{Backend, Profile},
    lifecycle_workload,
    measure::{Captured, FdProbe, LiveFds, Metric, capture},
    persistent_workload,
    profile_workload::{level_repeat, one_shot_rearm},
    ready_workload::ready_transaction,
    scenario::Scenario,
    wake_workload,
};

pub(crate) fn run<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    match scenario {
        scenario if scenario.is_construct() => construct::<B>(scenario, iterations, metric),
        Scenario::RegisterDelete => register_delete::<B>(scenario, iterations, metric),
        Scenario::Register64 => lifecycle_workload::register::<B>(
            scenario.event_capacity(),
            scenario.registration_capacity(),
            scenario.batch_size(),
            iterations,
            metric,
        ),
        Scenario::Delete64 => lifecycle_workload::delete::<B>(
            scenario.event_capacity(),
            scenario.registration_capacity(),
            scenario.batch_size(),
            iterations,
            metric,
        ),
        Scenario::EmptyWait => empty_wait::<B>(scenario, iterations, metric),
        Scenario::ReadySingle | Scenario::ReadyBatch64 | Scenario::ReadyBatch1024 => {
            ready_transaction::<B>(scenario, iterations, metric)
        }
        scenario if scenario.is_persistent() => persistent_workload::run::<B>(
            scenario.event_capacity(),
            scenario.registration_capacity(),
            scenario.batch_size(),
            iterations,
            metric,
        ),
        Scenario::WakeRoundtrip => wake_workload::pretriggered::<B>(
            scenario.event_capacity(),
            scenario.registration_capacity(),
            iterations,
            metric,
        ),
        Scenario::WakeBlocked => wake_workload::blocked::<B>(
            scenario.event_capacity(),
            scenario.registration_capacity(),
            iterations,
            metric,
        ),
        Scenario::LevelRepeat => level_repeat::<B>(scenario, iterations, metric),
        Scenario::OneShotRearm => one_shot_rearm::<B>(scenario, iterations, metric),
        _ => Err("scenario dispatch invariant failed".to_owned()),
    }
}

fn construct<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let probe = FdProbe::discover();
    let fixture_baseline = probe.count();
    let mut live = B::new(scenario.event_capacity(), scenario.registration_capacity())?;
    let candidate_setup = probe.count();
    let external_wake = if scenario.constructs_waker() {
        live.configure_wake()?;
        Some(live.wake_handle()?)
    } else {
        None
    };
    let active = probe.count();
    drop(external_wake);
    drop(live);
    let post_cleanup = probe.count();
    capture(iterations, metric, || {
        if scenario.constructs_waker() {
            B::construct_with_waker_once(
                scenario.event_capacity(),
                scenario.registration_capacity(),
            )?;
        } else {
            B::construct_once(scenario.event_capacity(), scenario.registration_capacity())?;
        }
        Ok(0)
    })
    .map(|captured| {
        captured.with_live_fds(LiveFds::from_options(
            fixture_baseline,
            candidate_setup,
            active,
            post_cleanup,
        ))
    })
}

fn register_delete<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let (source, _peer) = UnixStream::pair().map_err(display)?;
    let probe = FdProbe::discover();
    let fixture_baseline = probe.count();
    let mut backend = B::new(scenario.event_capacity(), scenario.registration_capacity())?;
    let candidate_setup = probe.count();
    let probe_registration = backend.register(&source, 0, Profile::InitialObservation)?;
    let active = probe.count();
    backend.delete(probe_registration)?;
    let post_cleanup = probe.count();
    capture(iterations, metric, || {
        let registration = backend.register(&source, 0, Profile::InitialObservation)?;
        backend.delete(registration)?;
        Ok(0)
    })
    .map(|captured| {
        captured.with_live_fds(LiveFds::from_options(
            fixture_baseline,
            candidate_setup,
            active,
            post_cleanup,
        ))
    })
}

fn empty_wait<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let probe = FdProbe::discover();
    let fixture_baseline = probe.count();
    let mut backend = B::new(scenario.event_capacity(), scenario.registration_capacity())?;
    let candidate_setup = probe.count();
    let active = candidate_setup;
    let captured = capture(iterations, metric, || {
        let observed = backend.wait(Duration::ZERO, &mut |_| {
            Err("empty wait produced a resource event".to_owned())
        })?;
        if observed == 0 {
            Ok(0)
        } else {
            Err(format!("empty wait returned {observed} events"))
        }
    });
    drop(backend);
    let post_cleanup = probe.count();
    captured.map(|value| {
        value.with_live_fds(LiveFds::from_options(
            fixture_baseline,
            candidate_setup,
            active,
            post_cleanup,
        ))
    })
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
