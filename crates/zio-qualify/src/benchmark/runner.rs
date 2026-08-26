//! Failure-retaining candidate rotation and receipt emission.

use std::io::Write;

use crate::Implementation;

use super::{
    candidate_bench::{self, Support},
    config::Config,
    measure::{Captured, FdProbe, Metric},
    metadata::Metadata,
    receipt,
    record::Sample,
    runner_prepare,
    runner_support::{
        CandidateSamples, context, emit_failure, fd_delta, write_line, write_passed_summary,
    },
    scenario::Scenario,
};

pub(crate) use super::runner_support::selected_scenarios;

pub(crate) trait Driver {
    fn support(implementation: Implementation, scenario: Scenario) -> Result<Support, String>;
    fn run(
        implementation: Implementation,
        scenario: Scenario,
        iterations: usize,
        metric: Option<Metric>,
    ) -> Result<Captured, String>;
}

pub(crate) struct NativeDriver;

impl Driver for NativeDriver {
    fn support(implementation: Implementation, scenario: Scenario) -> Result<Support, String> {
        candidate_bench::support(implementation, scenario)
    }

    fn run(
        implementation: Implementation,
        scenario: Scenario,
        iterations: usize,
        metric: Option<Metric>,
    ) -> Result<Captured, String> {
        candidate_bench::run(implementation, scenario, iterations, metric)
    }
}

pub(crate) fn execute(
    metric: Metric,
    config: &Config,
    metadata: &Metadata,
    output: &mut dyn Write,
    summary: &mut dyn Write,
) -> Result<(), String> {
    execute_with::<NativeDriver>(metric, config, metadata, output, summary)
}

pub(crate) fn execute_with<D: Driver>(
    metric: Metric,
    config: &Config,
    metadata: &Metadata,
    output: &mut dyn Write,
    summary: &mut dyn Write,
) -> Result<(), String> {
    if metric == Metric::Timing && cfg!(feature = "allocation-metrics") {
        return Err("instrumented builds cannot emit timing receipts".to_owned());
    }
    let fd_probe = FdProbe::discover();
    let mut failures = 0_usize;
    for scenario in selected_scenarios(config) {
        failures = failures.saturating_add(execute_scenario::<D>(
            metric, config, metadata, scenario, &fd_probe, output, summary,
        )?);
    }
    output.flush().map_err(display)?;
    summary.flush().map_err(display)?;
    if failures == 0 {
        Ok(())
    } else {
        Err(format!(
            "{failures} benchmark candidate failure(s); retained receipts contain the evidence"
        ))
    }
}

fn execute_scenario<D: Driver>(
    metric: Metric,
    config: &Config,
    metadata: &Metadata,
    scenario: Scenario,
    fd_probe: &FdProbe,
    output: &mut dyn Write,
    summary: &mut dyn Write,
) -> Result<usize, String> {
    let (mut active, mut failures) = runner_prepare::candidates::<D>(
        metric, config, metadata, scenario, fd_probe, output, summary,
    )?;
    failures = failures.saturating_add(measure_rounds::<D>(
        metric,
        config,
        metadata,
        scenario,
        fd_probe,
        &mut active,
        output,
        summary,
    )?);
    for candidate in active.into_iter().filter(|candidate| !candidate.failed) {
        let line = receipt::measurement(metric, metadata, config, &candidate, scenario, fd_probe)?;
        write_line(output, &line)?;
        write_passed_summary(
            metric,
            summary,
            candidate.implementation,
            scenario,
            &candidate.samples,
        )?;
    }
    Ok(failures)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the runner keeps receipt context explicit"
)]
fn measure_rounds<D: Driver>(
    metric: Metric,
    config: &Config,
    metadata: &Metadata,
    scenario: Scenario,
    fd_probe: &FdProbe,
    active: &mut [CandidateSamples],
    output: &mut dyn Write,
    summary: &mut dyn Write,
) -> Result<usize, String> {
    let mut failures = 0_usize;
    for round in 0..config.samples {
        for order_position in 0..active.len() {
            let index = (round + order_position) % active.len();
            if active[index].failed {
                continue;
            }
            let implementation = active[index].implementation;
            let before = fd_probe.count();
            match D::run(
                implementation,
                scenario,
                active[index].iterations,
                Some(metric),
            ) {
                Ok(captured) => active[index].samples.push(Sample {
                    round,
                    order_position,
                    captured,
                    retained_fd_delta: fd_delta(before, fd_probe.count()),
                }),
                Err(error) => {
                    active[index].failed = true;
                    emit_failure(
                        context(metric, metadata, config, implementation, scenario),
                        "measurement",
                        &error,
                        fd_probe,
                        &active[index].samples,
                        output,
                        summary,
                    )?;
                    failures = failures.saturating_add(1);
                }
            }
        }
    }
    Ok(failures)
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
