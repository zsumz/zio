//! Readiness cycles with registration lifecycle outside the measured region.

use super::{
    backend::{Backend, Profile},
    measure::{Captured, FdProbe, LiveFds, Metric},
    ready_workload::{collect_all, delete_all, pairs, register_all_with_profile, signal_all},
};

pub(crate) fn run<B: Backend>(
    event_capacity: usize,
    registration_capacity: usize,
    batch: usize,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let probe = FdProbe::discover();
    let (sources, peers) = pairs(batch)?;
    let fixture_baseline = probe.count();
    let mut backend = B::new(event_capacity, registration_capacity)?;
    let candidate_setup = probe.count();
    let mut registrations = Vec::with_capacity(batch);
    register_all_with_profile(
        &mut backend,
        &sources,
        &mut registrations,
        Profile::Persistent,
    )?;
    let active = probe.count();
    let mut seen = vec![false; batch];
    let measured = super::measure::capture(iterations, metric, || {
        signal_all(&peers)?;
        collect_all(&mut backend, &sources, &mut seen)?;
        u64::try_from(batch).map_err(display)
    });
    let cleanup = delete_all(&mut backend, &mut registrations);
    let post_cleanup = probe.count();
    combine(measured, cleanup).map(|captured| {
        captured.with_live_fds(LiveFds::from_options(
            fixture_baseline,
            candidate_setup,
            active,
            post_cleanup,
        ))
    })
}

fn combine(
    measured: Result<Captured, String>,
    cleanup: Result<(), String>,
) -> Result<Captured, String> {
    match (measured, cleanup) {
        (Ok(captured), Ok(())) => Ok(captured),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
