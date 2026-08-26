//! Candidate pilot and shared-iteration calibration receipt fields.

use super::{config::Config, json, runner_support::CandidateSamples, scenario::Scenario};

pub(crate) fn write(
    output: &mut String,
    config: &Config,
    scenario: Scenario,
    sampling: Option<&CandidateSamples>,
) {
    json::key(output, "calibration");
    output.push('{');
    let target_ns = u128::from(config.target_sample_time_ms).saturating_mul(1_000_000);
    let value = sampling.and_then(|candidate| candidate.calibration);
    optional(output, "target_sample_ns", value.map(|_| target_ns));
    optional(
        output,
        "probe_iterations",
        value.and_then(|item| u128::try_from(item.probe_iterations).ok()),
    );
    optional(
        output,
        "probe_elapsed_ns",
        value.map(|item| item.probe_elapsed_ns),
    );
    optional(
        output,
        "candidate_required_iterations",
        value.and_then(|item| u128::try_from(item.required_iterations).ok()),
    );
    optional(
        output,
        "pilot_achieved_elapsed_ns",
        value.map(|item| item.achieved_elapsed_ns),
    );
    optional(
        output,
        "maximum_iterations",
        value.and_then(|_| u128::try_from(scenario.max_calibrated_iterations()).ok()),
    );
    json::field_number(
        output,
        "selected_shared_iterations",
        sampling.map_or_else(|| config.iterations_for(scenario), |item| item.iterations),
        false,
    );
    output.push('}');
}

fn optional(output: &mut String, name: &str, value: Option<u128>) {
    json::key(output, name);
    match value {
        Some(value) => output.push_str(&value.to_string()),
        None => output.push_str("null"),
    }
    output.push(',');
}
