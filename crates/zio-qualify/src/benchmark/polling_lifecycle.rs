//! Direct borrowed `polling` lifecycle segments without Arc adapter overhead.

use polling::Poller;

use super::{
    measure::{Captured, FdProbe, LiveFds, Metric},
    polling_direct::{delete_all, pairs, register_all},
    scenario::Scenario,
    segment_measure::capture_segmented,
};

pub(crate) fn register_only(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    run(scenario, iterations, metric, SegmentKind::Register)
}

pub(crate) fn delete_only(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    run(scenario, iterations, metric, SegmentKind::Delete)
}

fn run(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
    kind: SegmentKind,
) -> Result<Captured, String> {
    let batch = scenario.batch_size();
    let probe = FdProbe::discover();
    let (sources, _peers) = pairs(batch)?;
    let fixture_baseline = probe.count();
    let poller = Poller::new().map_err(display)?;
    let candidate_setup = probe.count();
    let mut registrations = Vec::with_capacity(batch);
    let mut active = None;
    let captured = capture_segmented(
        iterations,
        u64::try_from(batch).map_err(display)?,
        metric,
        |segment| {
            if kind == SegmentKind::Register {
                segment.measure(|| register_all(&poller, &sources, &mut registrations))?;
                active = active.or_else(|| probe.count());
                delete_all(&mut registrations)?;
            } else {
                register_all(&poller, &sources, &mut registrations)?;
                active = active.or_else(|| probe.count());
                segment.measure(|| delete_all(&mut registrations))?;
            }
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum SegmentKind {
    Register,
    Delete,
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
