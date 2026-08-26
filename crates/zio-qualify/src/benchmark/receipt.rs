//! Stable passed, unsupported, and failed receipt encoding.

use crate::Implementation;

use super::{
    candidate_bench::disclosure,
    config::Config,
    json,
    measure::{FdProbe, Metric},
    metadata::Metadata,
    receipt_context, receipt_data, receipt_resources,
    record::Sample,
    resource_limit::Unsupported,
    runner_support::CandidateSamples,
    scenario::Scenario,
};

pub(crate) fn measurement(
    metric: Metric,
    metadata: &Metadata,
    config: &Config,
    candidate: &CandidateSamples,
    scenario: Scenario,
    fd_probe: &FdProbe,
) -> Result<String, String> {
    let mut output = begin(
        metric,
        "passed",
        metadata,
        config,
        candidate.implementation,
        scenario,
        Some(candidate),
    );
    receipt_data::raw(&mut output, metric, &candidate.samples)?;
    output.push(',');
    receipt_data::summary(&mut output, metric, &candidate.samples)?;
    output.push(',');
    receipt_resources::live_fds(&mut output, &candidate.samples);
    output.push(',');
    receipt_resources::retained_fds(&mut output, fd_probe, &candidate.samples);
    finish(&mut output, candidate.implementation);
    Ok(output)
}

pub(crate) fn unsupported(
    metric: Metric,
    metadata: &Metadata,
    config: &Config,
    implementation: Implementation,
    scenario: Scenario,
    reason: &Unsupported,
) -> String {
    let mut output = begin(
        metric,
        "unsupported",
        metadata,
        config,
        implementation,
        scenario,
        None,
    );
    json::field_string(&mut output, "reason_code", reason.code, true);
    json::field_string(&mut output, "reason", &reason.reason, true);
    unsupported_resources(&mut output, reason);
    finish(&mut output, implementation);
    output
}

pub(crate) fn failed(
    context: FailureContext<'_>,
    phase: &'static str,
    error: &str,
    fd_probe: &FdProbe,
    samples: &[Sample],
) -> Result<String, String> {
    let mut output = begin(
        context.metric,
        "failed",
        context.metadata,
        context.config,
        context.implementation,
        context.scenario,
        None,
    );
    json::key(&mut output, "failure");
    output.push('{');
    json::field_string(&mut output, "phase", phase, true);
    json::field_string(&mut output, "message", error, true);
    json::field_number(&mut output, "completed_samples", samples.len(), false);
    output.push_str("},");
    receipt_data::raw(&mut output, context.metric, samples)?;
    output.push(',');
    receipt_resources::live_fds(&mut output, samples);
    output.push(',');
    receipt_resources::retained_fds(&mut output, fd_probe, samples);
    finish(&mut output, context.implementation);
    Ok(output)
}

#[derive(Clone, Copy)]
pub(crate) struct FailureContext<'a> {
    pub(crate) metric: Metric,
    pub(crate) metadata: &'a Metadata,
    pub(crate) config: &'a Config,
    pub(crate) implementation: Implementation,
    pub(crate) scenario: Scenario,
}

fn begin(
    metric: Metric,
    status: &'static str,
    metadata: &Metadata,
    config: &Config,
    implementation: Implementation,
    scenario: Scenario,
    sampling: Option<&CandidateSamples>,
) -> String {
    let mut output =
        String::with_capacity(config.samples.saturating_mul(160).saturating_add(1_024));
    output.push('{');
    json::field_string(&mut output, "schema", "zio.perf.v2", true);
    json::field_string(&mut output, "kind", "measurement", true);
    json::field_string(&mut output, "metric", metric.name(), true);
    json::field_string(&mut output, "status", status, true);
    receipt_context::common(
        &mut output,
        metric,
        metadata,
        config,
        implementation,
        scenario,
        sampling,
    );
    output
}

fn finish(output: &mut String, implementation: Implementation) {
    output.push(',');
    json::field_string(output, "disclosure", disclosure(implementation), false);
    output.push('}');
}

fn unsupported_resources(output: &mut String, reason: &Unsupported) {
    json::key(output, "resource_requirement");
    output.push('{');
    optional_number(
        output,
        "required_additional_fds",
        reason.required_additional_fds,
    );
    optional_number(output, "observed_open_fds", reason.observed_open_fds);
    optional_number(
        output,
        "observed_soft_fd_limit",
        reason.observed_soft_fd_limit,
    );
    json::field_string(
        output,
        "fd_limit_source",
        reason.fd_limit_source.unwrap_or("unavailable"),
        false,
    );
    output.push('}');
}

fn optional_number(output: &mut String, name: &str, value: Option<u64>) {
    json::key(output, name);
    match value {
        Some(value) => output.push_str(&value.to_string()),
        None => output.push_str("null"),
    }
    output.push(',');
}
