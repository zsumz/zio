//! Receipt flush, rotation selection, and summary helpers.

use std::io::Write;

use crate::Implementation;

use super::{
    calibration::Calibration,
    config::Config,
    measure::{CapturedMetric, FdProbe, Metric, distribution},
    metadata::Metadata,
    receipt::{self, FailureContext},
    record::Sample,
    scenario::Scenario,
};

pub(crate) fn selected_scenarios(config: &Config) -> Vec<Scenario> {
    config.scenario.map_or_else(
        || {
            Scenario::ALL
                .into_iter()
                .filter(|scenario| {
                    !config.smoke
                        || !matches!(
                            scenario,
                            Scenario::ReadyBatch1024 | Scenario::PersistentBatch1024
                        )
                })
                .collect()
        },
        |scenario| vec![scenario],
    )
}

pub(crate) fn selected_implementations(config: &Config, scenario: Scenario) -> Vec<Implementation> {
    config.implementation.map_or_else(
        || {
            Implementation::ALL
                .into_iter()
                .filter(|item| scenario.supports(*item))
                .collect()
        },
        |implementation| vec![implementation],
    )
}

pub(crate) fn context<'a>(
    metric: Metric,
    metadata: &'a Metadata,
    config: &'a Config,
    implementation: Implementation,
    scenario: Scenario,
) -> FailureContext<'a> {
    FailureContext {
        metric,
        metadata,
        config,
        implementation,
        scenario,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "failure evidence keeps every receipt input explicit"
)]
pub(crate) fn emit_failure(
    context: FailureContext<'_>,
    phase: &'static str,
    error: &str,
    fd_probe: &FdProbe,
    samples: &[Sample],
    output: &mut dyn Write,
    summary: &mut dyn Write,
) -> Result<(), String> {
    let line = receipt::failed(context, phase, error, fd_probe, samples)?;
    write_line(output, &line)?;
    write_summary_line(
        summary,
        &format!(
            "{} {}: failed in {phase} ({error})",
            context.implementation.name(),
            context.scenario.name()
        ),
    )
}

pub(crate) fn write_passed_summary(
    metric: Metric,
    summary: &mut dyn Write,
    implementation: Implementation,
    scenario: Scenario,
    samples: &[Sample],
) -> Result<(), String> {
    let detail = match metric {
        Metric::Timing => {
            let values: Result<Vec<_>, _> = samples.iter().map(timing_per_operation).collect();
            let value = distribution(&values?)?;
            format!(
                "median_sample_mean={} ns/op p95_sample_mean={} ns/op mad_sample_mean={} ns/op",
                value.median, value.p95, value.mad
            )
        }
        Metric::Allocation => format!("allocation samples={}", samples.len()),
    };
    write_summary_line(
        summary,
        &format!("{} {}: {detail}", implementation.name(), scenario.name()),
    )
}

fn timing_per_operation(sample: &Sample) -> Result<u128, String> {
    match sample.captured.metric {
        CapturedMetric::Timing { elapsed_ns } => {
            Ok(elapsed_ns / u128::from(sample.captured.counts.operations.max(1)))
        }
        _ => Err("timing summary received a non-timing sample".to_owned()),
    }
}

pub(crate) fn write_line(output: &mut dyn Write, line: &str) -> Result<(), String> {
    output.write_all(line.as_bytes()).map_err(display)?;
    output.write_all(b"\n").map_err(display)?;
    output.flush().map_err(display)
}

pub(crate) fn write_summary_line(summary: &mut dyn Write, line: &str) -> Result<(), String> {
    writeln!(summary, "zio-perf {line}").map_err(display)?;
    summary.flush().map_err(display)
}

pub(crate) fn fd_delta(before: Option<usize>, after: Option<usize>) -> Option<i64> {
    let before = i128::try_from(before?).ok()?;
    let after = i128::try_from(after?).ok()?;
    i64::try_from(after - before).ok()
}

pub(crate) struct CandidateSamples {
    pub(crate) implementation: Implementation,
    pub(crate) samples: Vec<Sample>,
    pub(crate) failed: bool,
    pub(crate) calibration: Option<Calibration>,
    pub(crate) iterations: usize,
    pub(crate) warmup_iterations: usize,
}

impl CandidateSamples {
    pub(crate) fn new(
        implementation: Implementation,
        samples: usize,
        calibration: Option<Calibration>,
        iterations: usize,
    ) -> Self {
        Self {
            implementation,
            samples: Vec::with_capacity(samples),
            failed: false,
            calibration,
            iterations,
            warmup_iterations: 0,
        }
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
