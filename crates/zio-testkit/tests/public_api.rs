//! Stable public report and scenario vocabulary evidence.

use std::io;

use zio::CommitStatus;
use zio_testkit::{
    Branch, ConformanceCheck, ConformanceFailure, DELETE_UNKNOWN, MODIFY_APPLIED, MutationScenario,
    REGISTER_NOT_APPLIED, REGISTER_SUCCESS, run_all,
};

#[test]
fn full_report_covers_every_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_all();
    assert_eq!(report.len(), MutationScenario::ALL.len());
    assert_eq!(report.passed(), MutationScenario::ALL.len());
    assert!(!report.is_empty());
    assert!(report.is_conformant());
    assert_eq!(report.failures().count(), 0);
    for (result, scenario) in report.results().iter().zip(MutationScenario::ALL) {
        assert_eq!(result.scenario(), scenario);
        assert!(result.is_passed());
        assert_eq!(result.failure(), None);
    }
    let first = report
        .results()
        .first()
        .cloned()
        .ok_or_else(|| io::Error::other("missing first scenario"))?;
    assert_eq!(first.into_parts(), (REGISTER_SUCCESS, None));
    report.into_result()?;
    Ok(())
}

#[test]
fn scenario_names_and_constructors_are_stable() {
    assert_eq!(
        MutationScenario::ALL.map(MutationScenario::name),
        [
            "register.success",
            "register.not_applied",
            "register.applied",
            "register.unknown",
            "modify.success",
            "modify.not_applied",
            "modify.applied",
            "modify.unknown",
            "delete.success",
            "delete.not_applied",
            "delete.applied",
            "delete.unknown",
        ]
    );
    assert_eq!(
        MutationScenario::register(Branch::NotApplied),
        REGISTER_NOT_APPLIED
    );
    assert_eq!(MutationScenario::modify(Branch::Applied), MODIFY_APPLIED);
    assert_eq!(MutationScenario::delete(Branch::Unknown), DELETE_UNKNOWN);
    assert_eq!(Branch::Success.commit(), None);
    assert_eq!(Branch::NotApplied.commit(), Some(CommitStatus::NotApplied));
    assert_eq!(Branch::Applied.commit(), Some(CommitStatus::Applied));
    assert_eq!(Branch::Unknown.commit(), Some(CommitStatus::Unknown));
}

#[test]
fn conformance_failures_remain_structured() {
    let failure = ConformanceFailure::new(
        REGISTER_NOT_APPLIED,
        ConformanceCheck::IntoParts,
        "returned handle",
        "missing handle",
    );
    assert_eq!(failure.scenario(), REGISTER_NOT_APPLIED);
    assert_eq!(failure.check(), ConformanceCheck::IntoParts);
    assert_eq!(failure.expected(), "returned handle");
    assert_eq!(failure.actual(), "missing handle");
    assert!(failure.to_string().contains("register.not_applied"));
}
