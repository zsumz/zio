//! Coverage-policy proofs.

use std::io;

use crate::{
    CaseOutcome, CaseResult, Implementation, ProfileSupport, QualificationReport, Scenario,
};

const OMITTED_REASON: &str = "injected not-run case";

#[test]
fn an_all_not_run_matrix_is_not_conformant() -> Result<(), io::Error> {
    let mut results = Vec::new();
    for implementation in Implementation::ALL {
        for scenario in Scenario::ALL {
            results.push(CaseResult::new(
                implementation,
                scenario,
                None,
                Vec::new(),
                CaseOutcome::NotRun(ProfileSupport::NotExposed {
                    reason: OMITTED_REASON,
                }),
            ));
        }
    }
    let report = QualificationReport::for_all(results);
    check(
        !report.has_required_coverage(),
        "all-not-run matrix claimed required coverage",
    )?;
    check(
        !report.is_conformant(),
        "all-not-run matrix was declared conformant",
    )
}

#[test]
fn full_matrix_scope_rejects_one_implementation() -> Result<(), io::Error> {
    let report = QualificationReport::for_all(passing_zio_results());
    check(
        !report.has_required_coverage(),
        "full matrix accepted one implementation's cases",
    )
}

#[test]
fn implementation_scope_accepts_only_its_complete_cases() -> Result<(), io::Error> {
    let report =
        QualificationReport::for_implementation(Implementation::Zio, passing_zio_results());
    check(
        report.has_required_coverage(),
        "complete implementation scope lacked coverage",
    )?;
    check(
        report.is_conformant(),
        "complete implementation scope failed",
    )?;

    let wrong_candidate =
        QualificationReport::for_implementation(Implementation::Zio, passing_polling_results());
    check(
        !wrong_candidate.has_required_coverage(),
        "implementation scope accepted another candidate's cases",
    )
}

#[test]
fn implementation_scope_rejects_all_not_run() -> Result<(), io::Error> {
    let results = Scenario::ALL
        .into_iter()
        .map(|scenario| {
            CaseResult::new(
                Implementation::Zio,
                scenario,
                None,
                Vec::new(),
                CaseOutcome::NotRun(ProfileSupport::NotExposed {
                    reason: OMITTED_REASON,
                }),
            )
        })
        .collect();
    let report = QualificationReport::for_implementation(Implementation::Zio, results);
    check(
        !report.has_required_coverage(),
        "all-not-run implementation scope claimed coverage",
    )?;
    check(
        !report.is_conformant(),
        "all-not-run implementation scope was conformant",
    )
}

fn passing_zio_results() -> Vec<CaseResult> {
    Scenario::ALL
        .into_iter()
        .map(|scenario| {
            let delivery = match scenario.profile() {
                crate::DeliveryProfile::InitialObservation | crate::DeliveryProfile::Level => {
                    crate::ConfiguredDelivery::Level
                }
                crate::DeliveryProfile::OneShot => crate::ConfiguredDelivery::OneShot,
            };
            CaseResult::new(
                Implementation::Zio,
                scenario,
                Some(delivery),
                Vec::new(),
                CaseOutcome::Passed,
            )
        })
        .collect()
}

fn passing_polling_results() -> Vec<CaseResult> {
    Scenario::ALL
        .into_iter()
        .map(|scenario| {
            let delivery = match scenario.profile() {
                crate::DeliveryProfile::Level => crate::ConfiguredDelivery::Level,
                crate::DeliveryProfile::InitialObservation | crate::DeliveryProfile::OneShot => {
                    crate::ConfiguredDelivery::OneShot
                }
            };
            CaseResult::new(
                Implementation::Polling,
                scenario,
                Some(delivery),
                Vec::new(),
                CaseOutcome::Passed,
            )
        })
        .collect()
}

fn check(condition: bool, message: &'static str) -> Result<(), io::Error> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message))
    }
}
