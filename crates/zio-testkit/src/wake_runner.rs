//! Isolated black-box wake scenario execution.

use crate::{WakeCaseResult, WakeCheck, WakeFailure, WakeReport, WakeScenario};

/// Runs one wake scenario against zio's public poller API.
pub fn run_wake_scenario(scenario: WakeScenario) -> Result<(), WakeFailure> {
    match scenario {
        WakeScenario::SameKeyClones => crate::wake_config::same_key_clones(scenario),
        WakeScenario::ConflictingKey => crate::wake_config::conflicting_key(scenario),
        WakeScenario::PreWaitStorm => crate::wake_delivery::pre_wait_storm(scenario),
        WakeScenario::CloneAcrossWait => crate::wake_delivery::clone_across_wait(scenario),
        WakeScenario::MultiProducerStorm => crate::wake_concurrent::multi_producer_storm(scenario),
        WakeScenario::RepeatedCrossThread => {
            crate::wake_concurrent::repeated_cross_thread(scenario)
        }
        WakeScenario::CapacityOneSaturation => crate::wake_saturation::capacity_one(scenario),
    }
}

/// Runs every wake scenario against the host's native zio backend.
///
/// A downstream qualification test can retain the structured report instead of
/// relying on assertion panics:
///
/// ```
/// let report = zio_testkit::run_wake_conformance();
/// report.into_result()?;
/// # Ok::<(), zio_testkit::WakeReport>(())
/// ```
pub fn run_wake_conformance() -> WakeReport {
    let mut results = Vec::new();
    if results.try_reserve_exact(WakeScenario::ALL.len()).is_err() {
        return WakeReport::new(vec![WakeCaseResult::failed(WakeFailure::new(
            WakeScenario::SameKeyClones,
            WakeCheck::Setup,
            "result storage",
            "allocation failure",
        ))]);
    }
    for scenario in WakeScenario::ALL {
        results.push(match run_wake_scenario(scenario) {
            Ok(()) => WakeCaseResult::passed(scenario),
            Err(failure) => WakeCaseResult::failed(failure),
        });
    }
    WakeReport::new(results)
}
