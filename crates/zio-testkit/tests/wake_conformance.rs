//! Public black-box wake conformance evidence.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use zio_testkit::{
    WAKE_CAPACITY_ONE_SATURATION, WAKE_CLONE_ACROSS_WAIT, WAKE_CONFLICTING_KEY,
    WAKE_MULTI_PRODUCER_STORM, WAKE_PRE_WAIT_STORM, WAKE_REPEATED_CROSS_THREAD,
    WAKE_SAME_KEY_CLONES, WakeCheck, WakeFailure, WakeScenario, run_wake_conformance,
};

#[test]
fn wake_report_covers_every_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_wake_conformance();
    assert_eq!(report.len(), WakeScenario::ALL.len());
    assert_eq!(report.passed(), WakeScenario::ALL.len());
    assert!(!report.is_empty());
    assert!(report.is_conformant());
    assert_eq!(report.failures().count(), 0);
    for (result, scenario) in report.results().iter().zip(WakeScenario::ALL) {
        assert_eq!(result.scenario(), scenario);
        assert!(result.is_passed());
        assert_eq!(result.failure(), None);
    }
    let first = report
        .results()
        .first()
        .cloned()
        .ok_or("missing first wake scenario")?;
    assert_eq!(first.into_parts(), (WAKE_SAME_KEY_CLONES, None));
    report.into_result()?;
    Ok(())
}

#[test]
fn wake_scenario_names_are_stable() {
    assert_eq!(
        WakeScenario::ALL.map(WakeScenario::name),
        [
            "wake.same_key_clones",
            "wake.conflicting_key",
            "wake.pre_wait_storm",
            "wake.clone_across_wait",
            "wake.multi_producer_storm",
            "wake.repeated_cross_thread",
            "wake.capacity_one_saturation",
        ]
    );
    assert_eq!(WAKE_SAME_KEY_CLONES, WakeScenario::SameKeyClones);
    assert_eq!(WAKE_CONFLICTING_KEY, WakeScenario::ConflictingKey);
    assert_eq!(WAKE_PRE_WAIT_STORM, WakeScenario::PreWaitStorm);
    assert_eq!(WAKE_CLONE_ACROSS_WAIT, WakeScenario::CloneAcrossWait);
    assert_eq!(WAKE_MULTI_PRODUCER_STORM, WakeScenario::MultiProducerStorm);
    assert_eq!(
        WAKE_REPEATED_CROSS_THREAD,
        WakeScenario::RepeatedCrossThread
    );
    assert_eq!(
        WAKE_CAPACITY_ONE_SATURATION,
        WakeScenario::CapacityOneSaturation
    );
}

#[test]
fn wake_failures_remain_structured() {
    let failure = WakeFailure::new(
        WAKE_PRE_WAIT_STORM,
        WakeCheck::Drain,
        "empty destination",
        "retained wake",
    );
    assert_eq!(failure.scenario(), WAKE_PRE_WAIT_STORM);
    assert_eq!(failure.check(), WakeCheck::Drain);
    assert_eq!(failure.expected(), "empty destination");
    assert_eq!(failure.actual(), "retained wake");
    assert!(failure.to_string().contains("wake.pre_wait_storm"));
}
