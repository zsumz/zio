//! Benchmark execution, isolation, failure retention, and flush tests.

use std::io::{self, Write};

use crate::Implementation;

use super::{
    candidate_bench::Support,
    config::Config,
    measure::{Allocations, Captured, CapturedMetric, Counts, Metric, Resources},
    metadata,
    runner::{Driver, execute, execute_with, selected_scenarios},
    scenario::Scenario,
};

#[cfg(not(feature = "allocation-metrics"))]
#[test]
fn tiny_timing_smoke_emits_one_receipt() -> Result<(), String> {
    let config = config(Some(Implementation::Zio), Some(Scenario::EmptyWait));
    let mut output = Vec::new();
    let mut summary = Vec::new();
    execute(
        Metric::Timing,
        &config,
        &metadata::fixture(),
        &mut output,
        &mut summary,
    )?;
    let output = String::from_utf8(output).map_err(display)?;
    check(output.lines().count() == 1, "receipt count")?;
    check(output.contains("\"metric\":\"timing\""), "metric")?;
    check(output.contains("\"operations\":[1,1]"), "accounting")
}

#[cfg(feature = "allocation-metrics")]
#[test]
fn instrumented_runner_rejects_timing_before_output() -> Result<(), String> {
    let config = config(Some(Implementation::Zio), Some(Scenario::EmptyWait));
    let mut output = Vec::new();
    let mut summary = Vec::new();
    let error = execute(
        Metric::Timing,
        &config,
        &metadata::fixture(),
        &mut output,
        &mut summary,
    )
    .err()
    .ok_or_else(|| "instrumented timing unexpectedly ran".to_owned())?;
    check(error.contains("cannot emit timing"), "rejection")?;
    check(output.is_empty(), "timing receipt emitted")
}

#[test]
fn one_candidate_failure_is_retained_and_peers_continue() -> Result<(), String> {
    let config = config(None, Some(Scenario::EmptyWait));
    let mut output = FlushWriter::default();
    let mut summary = FlushWriter::default();
    let error = execute_with::<FailingMio>(
        selected_metric(),
        &config,
        &metadata::fixture(),
        &mut output,
        &mut summary,
    )
    .err()
    .ok_or_else(|| "candidate failure did not fail the command".to_owned())?;
    let receipt = String::from_utf8(output.bytes.clone()).map_err(display)?;
    check(
        error.contains("1 benchmark candidate failure"),
        "final error",
    )?;
    check(receipt.lines().count() == 4, "receipt count")?;
    check(receipt.contains("\"status\":\"failed\""), "failed receipt")?;
    check(
        receipt.contains("\"phase\":\"measurement\""),
        "failure phase",
    )?;
    check(
        receipt.contains("fixture candidate failure"),
        "failure detail",
    )?;
    check(
        receipt.contains("\"retained_fds\":{\"status\":\"unavailable\""),
        "zero-sample FD state",
    )?;
    check(
        passed_candidate(&receipt, Implementation::Zio),
        "zio did not continue",
    )?;
    check(
        passed_candidate(&receipt, Implementation::ZioBorrowed),
        "zio borrowed did not continue",
    )?;
    check(
        passed_candidate(&receipt, Implementation::Polling),
        "polling did not continue",
    )?;
    check(output.flushes >= 3, "output was not flushed per receipt")?;
    check(summary.flushes >= 3, "summary was not flushed")
}

#[test]
fn unfiltered_smoke_excludes_the_descriptor_heavy_batch() -> Result<(), String> {
    let selected = selected_scenarios(&config(None, None));
    check(
        !selected.contains(&Scenario::ReadyBatch1024),
        "descriptor-heavy smoke scenario",
    )?;
    check(
        !selected.contains(&Scenario::PersistentBatch1024),
        "persistent descriptor-heavy smoke scenario",
    )?;
    check(
        selected.contains(&Scenario::ReadyBatch64),
        "small batch omitted",
    )
}

struct FailingMio;

impl Driver for FailingMio {
    fn support(_implementation: Implementation, _scenario: Scenario) -> Result<Support, String> {
        Ok(Support::Available)
    }

    fn run(
        implementation: Implementation,
        _scenario: Scenario,
        iterations: usize,
        metric: Option<Metric>,
    ) -> Result<Captured, String> {
        if implementation == Implementation::Mio && metric.is_some() {
            return Err("fixture candidate failure".to_owned());
        }
        Ok(Captured {
            counts: Counts {
                operations: u64::try_from(iterations).map_err(display)?,
                events: 0,
            },
            metric: captured_metric(metric),
            resources: Resources::default(),
        })
    }
}

fn captured_metric(metric: Option<Metric>) -> CapturedMetric {
    match metric {
        None => CapturedMetric::Warmup,
        Some(Metric::Timing) => CapturedMetric::Timing { elapsed_ns: 10 },
        Some(Metric::Allocation) => CapturedMetric::Allocation(Allocations::default()),
    }
}

const fn selected_metric() -> Metric {
    if cfg!(feature = "allocation-metrics") {
        Metric::Allocation
    } else {
        Metric::Timing
    }
}

fn config(implementation: Option<Implementation>, scenario: Option<Scenario>) -> Config {
    Config {
        samples: 2,
        iterations: None,
        warmup_iterations: 1,
        warmup_explicit: false,
        target_sample_time_ms: 100,
        implementation,
        scenario,
        output: None,
        help: false,
        smoke: true,
    }
}

fn passed_candidate(receipt: &str, implementation: Implementation) -> bool {
    receipt.lines().any(|line| {
        line.contains("\"status\":\"passed\"")
            && line.contains(&format!("\"name\":\"{}\"", implementation.name()))
    })
}

#[derive(Default)]
struct FlushWriter {
    bytes: Vec<u8>,
    flushes: usize,
}

impl Write for FlushWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes = self.flushes.saturating_add(1);
        Ok(())
    }
}

fn check(condition: bool, message: &'static str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_owned())
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
