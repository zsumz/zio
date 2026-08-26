//! Candidate calibration for equal-duration, equal-iteration timing rounds.

use super::measure::{Captured, CapturedMetric};

const MIN_PROBE_NS: u128 = 10_000_000;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Calibration {
    pub(crate) probe_iterations: usize,
    pub(crate) probe_elapsed_ns: u128,
    pub(crate) required_iterations: usize,
    pub(crate) achieved_elapsed_ns: u128,
}

pub(crate) fn calibrate(
    seed: usize,
    target_sample_ns: u128,
    maximum_iterations: usize,
    mut run: impl FnMut(usize) -> Result<Captured, String>,
) -> Result<Calibration, String> {
    let maximum_iterations = maximum_iterations.max(1);
    let mut iterations = seed.clamp(1, maximum_iterations);
    let (probe_iterations, probe_elapsed_ns) = loop {
        let elapsed = elapsed(run(iterations)?)?;
        if elapsed >= MIN_PROBE_NS || iterations == maximum_iterations {
            break (iterations, elapsed);
        }
        let growth = MIN_PROBE_NS.div_ceil(elapsed.max(1)).clamp(2, 16);
        let growth = usize::try_from(growth).map_err(display)?;
        let next = iterations.saturating_mul(growth).min(maximum_iterations);
        if next == iterations {
            break (iterations, elapsed);
        }
        iterations = next;
    };
    let required_iterations = scaled_iterations(
        probe_iterations,
        probe_elapsed_ns,
        target_sample_ns,
        maximum_iterations,
    )?;
    let achieved_elapsed_ns = elapsed(run(required_iterations)?)?;
    Ok(Calibration {
        probe_iterations,
        probe_elapsed_ns,
        required_iterations,
        achieved_elapsed_ns,
    })
}

fn scaled_iterations(
    iterations: usize,
    elapsed_ns: u128,
    target_ns: u128,
    maximum_iterations: usize,
) -> Result<usize, String> {
    let iterations = u128::try_from(iterations).map_err(display)?;
    let scaled = iterations
        .checked_mul(target_ns)
        .ok_or_else(|| "calibration scale overflow".to_owned())?
        .div_ceil(elapsed_ns.max(1))
        .clamp(1, maximum_iterations as u128);
    usize::try_from(scaled).map_err(display)
}

fn elapsed(captured: Captured) -> Result<u128, String> {
    match captured.metric {
        CapturedMetric::Timing { elapsed_ns } if elapsed_ns > 0 => Ok(elapsed_ns),
        CapturedMetric::Timing { .. } => Ok(1),
        _ => Err("calibration received a non-timing capture".to_owned()),
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
