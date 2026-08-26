//! Candidate preflight, calibration, shared iteration selection, and warmup.

use std::io::Write;

use super::{
    calibration::calibrate,
    candidate_bench::Support,
    config::Config,
    measure::{FdProbe, Metric},
    metadata::Metadata,
    receipt,
    runner::Driver,
    runner_support::{
        CandidateSamples, context, emit_failure, selected_implementations, write_line,
        write_summary_line,
    },
    scenario::Scenario,
};

pub(crate) fn candidates<D: Driver>(
    metric: Metric,
    config: &Config,
    metadata: &Metadata,
    scenario: Scenario,
    fd_probe: &FdProbe,
    output: &mut dyn Write,
    summary: &mut dyn Write,
) -> Result<(Vec<CandidateSamples>, usize), String> {
    let (mut active, mut failures) = preflight::<D>(
        metric, config, metadata, scenario, fd_probe, output, summary,
    )?;
    if config.calibrates(metric, scenario) {
        failures = failures.saturating_add(calibrate_candidates::<D>(
            config,
            metadata,
            scenario,
            fd_probe,
            &mut active,
            output,
            summary,
        )?);
    }
    let shared = active
        .iter()
        .filter(|candidate| !candidate.failed)
        .map(|candidate| candidate.iterations)
        .max()
        .unwrap_or_else(|| config.iterations_for(scenario));
    for candidate in &mut active {
        candidate.iterations = shared;
    }
    failures = failures.saturating_add(warm::<D>(
        metric,
        config,
        metadata,
        scenario,
        fd_probe,
        &mut active,
        output,
        summary,
    )?);
    Ok((active, failures))
}

#[allow(
    clippy::too_many_arguments,
    reason = "preflight retains complete receipt context"
)]
fn preflight<D: Driver>(
    metric: Metric,
    config: &Config,
    metadata: &Metadata,
    scenario: Scenario,
    fd_probe: &FdProbe,
    output: &mut dyn Write,
    summary: &mut dyn Write,
) -> Result<(Vec<CandidateSamples>, usize), String> {
    let mut active = Vec::new();
    let mut failures = 0_usize;
    for implementation in selected_implementations(config, scenario) {
        match D::support(implementation, scenario) {
            Ok(Support::Available) => active.push(CandidateSamples::new(
                implementation,
                config.samples,
                None,
                config.iterations_for(scenario),
            )),
            Ok(Support::Unavailable(reason)) => {
                write_line(
                    output,
                    &receipt::unsupported(
                        metric,
                        metadata,
                        config,
                        implementation,
                        scenario,
                        &reason,
                    ),
                )?;
                write_summary_line(
                    summary,
                    &format!(
                        "{} {}: unsupported ({})",
                        implementation.name(),
                        scenario.name(),
                        reason.reason
                    ),
                )?;
            }
            Err(error) => {
                emit_failure(
                    context(metric, metadata, config, implementation, scenario),
                    "preflight",
                    &error,
                    fd_probe,
                    &[],
                    output,
                    summary,
                )?;
                failures = failures.saturating_add(1);
            }
        }
    }
    Ok((active, failures))
}

#[allow(
    clippy::too_many_arguments,
    reason = "calibration failures retain full context"
)]
fn calibrate_candidates<D: Driver>(
    config: &Config,
    metadata: &Metadata,
    scenario: Scenario,
    fd_probe: &FdProbe,
    active: &mut [CandidateSamples],
    output: &mut dyn Write,
    summary: &mut dyn Write,
) -> Result<usize, String> {
    let mut failures = 0_usize;
    let target_ns = u128::from(config.target_sample_time_ms).saturating_mul(1_000_000);
    for candidate in active {
        let implementation = candidate.implementation;
        match calibrate(
            scenario.default_iterations(),
            target_ns,
            scenario.max_calibrated_iterations(),
            |iterations| D::run(implementation, scenario, iterations, Some(Metric::Timing)),
        ) {
            Ok(calibration) => {
                candidate.iterations = calibration.required_iterations;
                candidate.calibration = Some(calibration);
            }
            Err(error) => {
                candidate.failed = true;
                emit_failure(
                    context(Metric::Timing, metadata, config, implementation, scenario),
                    "calibration",
                    &error,
                    fd_probe,
                    &[],
                    output,
                    summary,
                )?;
                failures = failures.saturating_add(1);
            }
        }
    }
    Ok(failures)
}

#[allow(
    clippy::too_many_arguments,
    reason = "warmup failures retain full context"
)]
fn warm<D: Driver>(
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
    for candidate in active.iter_mut().filter(|candidate| !candidate.failed) {
        let iterations = config.warmup_for(metric, scenario, candidate.iterations);
        candidate.warmup_iterations = iterations;
        if let Err(error) = D::run(candidate.implementation, scenario, iterations, None) {
            candidate.failed = true;
            emit_failure(
                context(metric, metadata, config, candidate.implementation, scenario),
                "warmup",
                &error,
                fd_probe,
                &[],
                output,
                summary,
            )?;
            failures = failures.saturating_add(1);
        }
    }
    Ok(failures)
}
