//! Isolated black-box readiness scenario execution.

use crate::{
    ReadinessCaseResult, ReadinessCheck, ReadinessFailure, ReadinessFixture, ReadinessReport,
    ReadinessScenario,
};

/// Runs one readiness scenario against zio's public native poller API.
pub fn run_readiness_scenario(scenario: ReadinessScenario) -> Result<(), ReadinessFailure> {
    match scenario.fixture() {
        ReadinessFixture::UnixPendingEof => crate::readiness_stream::unix_pending_eof(scenario),
        ReadinessFixture::UnixWritable => crate::readiness_stream::unix_writable(scenario),
        ReadinessFixture::TcpPendingEof => crate::readiness_stream::tcp_pending_eof(scenario),
        ReadinessFixture::PipePendingEof => crate::readiness_pipe::pending_eof(scenario),
        ReadinessFixture::PipeReaderClosed => crate::readiness_pipe::reader_closed(scenario),
    }
}

/// Runs every V1 readiness scenario against the host's native zio backend.
///
/// The suite uses only zio's ordinary public API. It checks the required
/// portable minimum, rejects hints outside each declared allowance, confirms
/// the associated I/O operation, and verifies exact level or one-shot state.
///
/// ```
/// let report = zio_testkit::run_readiness_conformance();
/// report.into_result()?;
/// # Ok::<(), zio_testkit::ReadinessReport>(())
/// ```
pub fn run_readiness_conformance() -> ReadinessReport {
    let mut results = Vec::new();
    if results
        .try_reserve_exact(ReadinessScenario::ALL.len())
        .is_err()
    {
        return ReadinessReport::new(vec![ReadinessCaseResult::failed(ReadinessFailure::new(
            ReadinessScenario::UnixPendingEofReadableLevel,
            ReadinessCheck::Setup,
            "result storage",
            "allocation failure",
        ))]);
    }
    for scenario in ReadinessScenario::ALL {
        results.push(match run_readiness_scenario(scenario) {
            Ok(()) => ReadinessCaseResult::passed(scenario),
            Err(failure) => ReadinessCaseResult::failed(failure),
        });
    }
    ReadinessReport::new(results)
}
