//! Receipt metric isolation, provenance, and accounting tests.

use crate::Implementation;

use super::{
    candidate_bench::version,
    config::Config,
    measure::{Allocations, Captured, CapturedMetric, Counts, FdProbe, Metric},
    metadata, receipt,
    record::Sample,
    resource_limit::Unsupported,
    scenario::Scenario,
};

#[test]
fn timing_receipt_has_timing_and_no_allocation_fields() -> Result<(), String> {
    let receipt = measurement(Metric::Timing, CapturedMetric::Timing { elapsed_ns: 640 })?;
    check(receipt.contains("\"metric\":\"timing\""), "metric")?;
    check(receipt.contains("\"elapsed_ns\":[640]"), "elapsed")?;
    check(
        receipt.contains("\"ns_per_operation\":[320]"),
        "per operation",
    )?;
    check(!receipt.contains("allocation_"), "allocation field leaked")?;
    check(
        !receipt.contains("allocation-counter"),
        "allocator version leaked",
    )
}

#[test]
fn allocation_receipt_has_allocations_and_no_timing_fields() -> Result<(), String> {
    let receipt = measurement(
        Metric::Allocation,
        CapturedMetric::Allocation(Allocations {
            count_total: 3,
            count_current: 0,
            count_peak: 2,
            bytes_total: 96,
            bytes_current: 0,
            bytes_peak: 64,
        }),
    )?;
    check(receipt.contains("\"metric\":\"allocation\""), "metric")?;
    check(
        receipt.contains("\"events\":128,\"allocation_count_total\":3"),
        "summary separator",
    )?;
    check(
        receipt.contains("\"allocation_bytes_total\":[96]"),
        "raw allocations",
    )?;
    check(!receipt.contains("elapsed_ns"), "elapsed leaked")?;
    check(
        !receipt.contains("ns_per_operation"),
        "timing summary leaked",
    )
}

#[test]
fn source_and_exact_versions_are_stable() -> Result<(), String> {
    let receipt = measurement(Metric::Timing, CapturedMetric::Timing { elapsed_ns: 640 })?;
    check(receipt.contains("\"dirty\":false"), "source state")?;
    check(
        receipt.contains(&format!("\"zio\":\"{}\"", env!("CARGO_PKG_VERSION"))),
        "Zio version",
    )?;
    check(
        version(Implementation::Zio) == env!("CARGO_PKG_VERSION"),
        "Zio package metadata",
    )
}

#[test]
fn receipt_records_scenario_specific_wait_and_absence_windows() -> Result<(), String> {
    let no_wait = measurement_for(
        Metric::Timing,
        CapturedMetric::Timing { elapsed_ns: 10 },
        Scenario::ConstructDrop,
    )?;
    check(no_wait.contains("\"wait_timeout_ms\":null"), "no wait")?;
    let nonblocking = measurement_for(
        Metric::Timing,
        CapturedMetric::Timing { elapsed_ns: 10 },
        Scenario::EmptyWait,
    )?;
    check(
        nonblocking.contains("\"wait_timeout_ms\":0"),
        "nonblocking wait",
    )?;
    let disarm = measurement_for(
        Metric::Timing,
        CapturedMetric::Timing { elapsed_ns: 10 },
        Scenario::OneShotDisarm,
    )?;
    check(
        disarm.contains(
            "\"wait_timeout_ms\":1000,\"absence_window_ms\":2,\"absence_window_timed\":true",
        ),
        "timed absence metadata",
    )?;
    let rearm = measurement_for(
        Metric::Timing,
        CapturedMetric::Timing { elapsed_ns: 10 },
        Scenario::OneShotRearm,
    )?;
    check(
        rearm.contains("\"absence_window_ms\":2,\"absence_window_timed\":false"),
        "untimed absence metadata",
    )
}

#[test]
fn unavailable_fd_limit_is_structured() -> Result<(), String> {
    let reason = Unsupported {
        code: "insufficient_fd_limit",
        reason: "fixture limit".to_owned(),
        required_additional_fds: Some(3_080),
        observed_open_fds: Some(12),
        observed_soft_fd_limit: Some(256),
        fd_limit_source: Some("fixture"),
    };
    let line = receipt::unsupported(
        Metric::Timing,
        &metadata::fixture(),
        &config(Scenario::ReadyBatch1024),
        Implementation::Zio,
        Scenario::ReadyBatch1024,
        &reason,
    );
    check(
        line.contains("\"reason_code\":\"insufficient_fd_limit\""),
        "reason code",
    )?;
    check(
        line.contains("\"required_additional_fds\":3080"),
        "required descriptors",
    )?;
    check(
        line.contains("\"observed_soft_fd_limit\":256"),
        "observed limit",
    )
}

fn measurement(metric: Metric, captured_metric: CapturedMetric) -> Result<String, String> {
    measurement_for(metric, captured_metric, Scenario::ReadyBatch64)
}

fn measurement_for(
    metric: Metric,
    captured_metric: CapturedMetric,
    scenario: Scenario,
) -> Result<String, String> {
    let samples = [Sample {
        round: 0,
        order_position: 2,
        captured: Captured {
            counts: Counts {
                operations: 2,
                events: 128,
            },
            metric: captured_metric,
        },
        retained_fd_delta: Some(0),
    }];
    receipt::measurement(
        metric,
        &metadata::fixture(),
        &config(scenario),
        Implementation::Zio,
        scenario,
        &FdProbe::Unavailable("fixture"),
        &samples,
    )
}

fn config(scenario: Scenario) -> Config {
    Config {
        samples: 1,
        iterations: None,
        warmup_iterations: 1,
        implementation: None,
        scenario: Some(scenario),
        output: None,
        help: false,
        smoke: false,
    }
}

fn check(condition: bool, message: &'static str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_owned())
}
