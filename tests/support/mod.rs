//! Shared integration-test assertions.

use zio::{RecoveryFailure, WaitReport};

pub(crate) fn require_no_recovery(report: WaitReport) -> Result<(), RecoveryFailure> {
    match report.into_recovery() {
        Some(failure) => Err(failure),
        None => Ok(()),
    }
}
