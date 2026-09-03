//! Post-delivery wait-report ownership regressions.

use std::io;

use crate::{CommitStatus, Operation, RecoveryFailure, RecoveryOutcome, Registration, WaitReport};

#[test]
fn complete_report_contains_no_recovery() {
    let report = WaitReport::new(None);

    assert!(report.is_complete());
    assert!(report.recovery().is_none());
    assert!(report.into_recovery().is_none());
}

#[test]
fn recovery_report_exposes_and_returns_its_owned_failure() -> Result<(), Box<dyn std::error::Error>>
{
    let registration = Registration::test(7);
    let report = WaitReport::new(Some(RecoveryFailure::new(
        Operation::Disarm,
        vec![RecoveryOutcome::new(registration, CommitStatus::Unknown)],
        io::Error::from_raw_os_error(5),
    )));

    assert!(!report.is_complete());
    let recovery = report
        .recovery()
        .ok_or_else(|| io::Error::other("missing recovery report"))?;
    assert_eq!(recovery.outcomes()[0].registration(), registration);

    let recovery = report
        .into_recovery()
        .ok_or_else(|| io::Error::other("missing owned recovery report"))?;
    assert_eq!(recovery.source().raw_os_error(), Some(5));
    Ok(())
}
