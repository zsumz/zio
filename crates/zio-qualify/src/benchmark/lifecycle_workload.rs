//! Segmented registration and deletion measurement with reusable fixtures.

use super::{
    backend::{Backend, Profile},
    measure::{Captured, FdProbe, LiveFds, Metric},
    ready_workload::{delete_all, pairs, register_all_with_profile},
    segment_measure::capture_segmented,
};

pub(crate) fn register<B: Backend>(
    event_capacity: usize,
    registration_capacity: usize,
    batch: usize,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let probe = FdProbe::discover();
    let (sources, _peers) = pairs(batch)?;
    let fixture_baseline = probe.count();
    let mut backend = B::new(event_capacity, registration_capacity)?;
    let candidate_setup = probe.count();
    let mut registrations = Vec::with_capacity(batch);
    let mut active = None;
    let captured = capture_segmented(
        iterations,
        u64::try_from(batch).map_err(display)?,
        metric,
        |segment| {
            segment.measure(|| {
                register_all_with_profile(
                    &mut backend,
                    &sources,
                    &mut registrations,
                    Profile::InitialObservation,
                )
            })?;
            active = active.or_else(|| probe.count());
            delete_all(&mut backend, &mut registrations)?;
            Ok(0)
        },
    )?;
    let post_cleanup = probe.count();
    Ok(captured.with_live_fds(LiveFds::from_options(
        fixture_baseline,
        candidate_setup,
        active,
        post_cleanup,
    )))
}

pub(crate) fn delete<B: Backend>(
    event_capacity: usize,
    registration_capacity: usize,
    batch: usize,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let probe = FdProbe::discover();
    let (sources, _peers) = pairs(batch)?;
    let fixture_baseline = probe.count();
    let mut backend = B::new(event_capacity, registration_capacity)?;
    let candidate_setup = probe.count();
    let mut registrations = Vec::with_capacity(batch);
    let mut active = None;
    let captured = capture_segmented(
        iterations,
        u64::try_from(batch).map_err(display)?,
        metric,
        |segment| {
            register_all_with_profile(
                &mut backend,
                &sources,
                &mut registrations,
                Profile::InitialObservation,
            )?;
            active = active.or_else(|| probe.count());
            segment.measure(|| delete_all(&mut backend, &mut registrations))?;
            Ok(0)
        },
    )?;
    let post_cleanup = probe.count();
    Ok(captured.with_live_fds(LiveFds::from_options(
        fixture_baseline,
        candidate_setup,
        active,
        post_cleanup,
    )))
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
