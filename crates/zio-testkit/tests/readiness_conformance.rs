//! Public black-box native readiness conformance evidence.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use zio::{Interest, Mode};
use zio_testkit::{
    ReadinessCheck, ReadinessFailure, ReadinessFixture, ReadinessScenario,
    run_readiness_conformance,
};

#[test]
fn readiness_report_covers_every_scenario() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_readiness_conformance();
    assert_eq!(report.len(), ReadinessScenario::ALL.len());
    assert_eq!(report.passed(), ReadinessScenario::ALL.len(), "{report}");
    assert!(!report.is_empty());
    assert!(report.is_conformant(), "{report}");
    assert_eq!(report.failures().count(), 0);
    for (result, scenario) in report.results().iter().zip(ReadinessScenario::ALL) {
        assert_eq!(result.scenario(), scenario);
        assert!(result.is_passed());
        assert_eq!(result.failure(), None);
    }
    let first = report
        .results()
        .first()
        .cloned()
        .ok_or("missing first readiness scenario")?;
    assert_eq!(
        first.into_parts(),
        (ReadinessScenario::UnixPendingEofReadableLevel, None)
    );
    report.into_result()?;
    Ok(())
}

#[test]
fn readiness_scenario_names_are_stable() {
    assert_eq!(
        ReadinessScenario::ALL.map(ReadinessScenario::name),
        [
            "readiness.unix.pending_eof.readable.level",
            "readiness.unix.pending_eof.readable.one_shot",
            "readiness.unix.pending_eof.combined.level",
            "readiness.unix.pending_eof.combined.one_shot",
            "readiness.unix.writable.level",
            "readiness.unix.writable.one_shot",
            "readiness.tcp.pending_eof.readable.level",
            "readiness.tcp.pending_eof.readable.one_shot",
            "readiness.pipe.pending_eof.readable.level",
            "readiness.pipe.pending_eof.readable.one_shot",
            "readiness.pipe.reader_closed.writable.level",
            "readiness.pipe.reader_closed.writable.one_shot",
        ]
    );
}

#[test]
fn readiness_scenario_parameters_cover_the_portable_matrix() {
    let combined = Interest::READABLE.union(Interest::WRITABLE);
    assert_eq!(
        ReadinessScenario::UnixPendingEofCombinedOneShot.interest(),
        combined
    );
    assert_eq!(
        ReadinessScenario::PipeReaderClosedLevel.interest(),
        Interest::WRITABLE
    );
    assert_eq!(
        ReadinessScenario::PipePendingEofOneShot.interest(),
        Interest::READABLE
    );
    assert_eq!(ReadinessScenario::TcpPendingEofLevel.mode(), Mode::Level);
    assert_eq!(
        ReadinessScenario::TcpPendingEofOneShot.mode(),
        Mode::OneShot
    );
    assert_eq!(
        ReadinessScenario::PipePendingEofLevel.fixture(),
        ReadinessFixture::PipePendingEof
    );
    assert_eq!(
        ReadinessScenario::ALL
            .iter()
            .filter(|scenario| scenario.mode() == Mode::Level)
            .count(),
        6
    );
    assert_eq!(
        ReadinessScenario::ALL
            .iter()
            .filter(|scenario| scenario.mode() == Mode::OneShot)
            .count(),
        6
    );
}

#[test]
fn readiness_failures_remain_structured() {
    let scenario = ReadinessScenario::PipeReaderClosedOneShot;
    let failure = ReadinessFailure::new(
        scenario,
        ReadinessCheck::AllowedReadiness,
        "documented readiness hints",
        "unexpected READABLE",
    );
    assert_eq!(failure.scenario(), scenario);
    assert_eq!(failure.check(), ReadinessCheck::AllowedReadiness);
    assert_eq!(failure.expected(), "documented readiness hints");
    assert_eq!(failure.actual(), "unexpected READABLE");
    assert!(failure.to_string().contains("readiness.pipe.reader_closed"));
}
