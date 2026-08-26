//! Shared provenance, candidate, scenario, and parameter receipt fields.

use crate::Implementation;

use super::{
    candidate_bench::version, config::Config, json, measure::Metric, metadata::Metadata,
    scenario::Scenario,
};

pub(crate) fn common(
    output: &mut String,
    metric: Metric,
    metadata: &Metadata,
    config: &Config,
    implementation: Implementation,
    scenario: Scenario,
) {
    git(output, metadata);
    output.push(',');
    host(output, metadata);
    output.push(',');
    toolchain(output, metadata);
    output.push(',');
    candidate(output, metric, metadata, implementation);
    output.push(',');
    scenario_json(output, scenario, implementation);
    output.push(',');
    parameters(output, config, scenario);
    output.push(',');
}

fn git(output: &mut String, metadata: &Metadata) {
    json::key(output, "git");
    output.push('{');
    json::field_string(output, "sha", &metadata.git_sha, true);
    json::field_string(output, "source", metadata.git_sha_source, true);
    json::key(output, "dirty");
    match metadata.git_dirty {
        Some(dirty) => output.push_str(if dirty { "true" } else { "false" }),
        None => output.push_str("null"),
    }
    output.push('}');
}

fn host(output: &mut String, metadata: &Metadata) {
    json::key(output, "host");
    output.push('{');
    json::field_string(output, "os", metadata.os, true);
    json::field_string(output, "os_version", &metadata.os_version, true);
    json::field_string(output, "arch", metadata.arch, true);
    json::field_string(output, "cpu", &metadata.cpu, true);
    json::field_number(output, "recorded_unix_ms", metadata.recorded_unix_ms, false);
    output.push('}');
}

fn toolchain(output: &mut String, metadata: &Metadata) {
    json::key(output, "toolchain");
    output.push('{');
    json::field_string(output, "rustc", &metadata.rustc, true);
    json::field_string(output, "rustflags", &metadata.rustflags, true);
    json::field_string(output, "build_profile", metadata.build_profile, false);
    output.push('}');
}

fn candidate(
    output: &mut String,
    metric: Metric,
    metadata: &Metadata,
    implementation: Implementation,
) {
    json::key(output, "candidate");
    output.push('{');
    json::field_string(output, "name", implementation.name(), true);
    json::field_string(output, "version", version(implementation), false);
    output.push_str("},");
    json::key(output, "harness");
    output.push('{');
    json::field_string(output, "crate", "zio-qualify", true);
    json::field_string(
        output,
        "version",
        metadata.qualify_version,
        metric == Metric::Allocation,
    );
    if metric == Metric::Allocation {
        json::field_string(output, "allocation_counter", "0.8.1", false);
    }
    output.push_str("},");
    json::key(output, "comparison_set");
    output.push('{');
    json::field_string(output, "zio", version(Implementation::Zio), true);
    json::field_string(output, "mio", version(Implementation::Mio), true);
    json::field_string(output, "polling", version(Implementation::Polling), false);
    output.push('}');
}

fn scenario_json(output: &mut String, scenario: Scenario, implementation: Implementation) {
    json::key(output, "scenario");
    output.push('{');
    json::field_string(output, "name", scenario.name(), true);
    json::field_string(output, "semantic_scope", scenario.semantic_scope(), true);
    json::field_string(
        output,
        "measurement_scope",
        scenario.measurement_scope(),
        true,
    );
    json::field_string(output, "delivery", scenario.delivery(), true);
    json::field_string(
        output,
        "candidate_setup",
        scenario.candidate_setup(implementation),
        true,
    );
    json::field_number(output, "batch_size", scenario.batch_size(), true);
    json::field_number(output, "event_capacity", scenario.event_capacity(), true);
    json::field_number(
        output,
        "registration_capacity",
        scenario.registration_capacity(),
        true,
    );
    optional_number(output, "wait_timeout_ms", scenario.wait_timeout_ms());
    optional_number(output, "absence_window_ms", scenario.absence_window_ms());
    optional_bool(
        output,
        "absence_window_timed",
        scenario.absence_window_timed(),
    );
    output.push('}');
}

fn parameters(output: &mut String, config: &Config, scenario: Scenario) {
    json::key(output, "parameters");
    output.push('{');
    json::field_number(output, "samples", config.samples, true);
    json::field_number(output, "iterations", config.iterations_for(scenario), true);
    json::field_string(
        output,
        "iterations_source",
        config.iterations_source(),
        true,
    );
    json::field_number(output, "warmup_iterations", config.warmup_iterations, true);
    json::field_string(output, "candidate_order", "rotate_left_by_round", true);
    json::field_string(output, "operation_unit", "scenario_iteration", false);
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

fn optional_bool(output: &mut String, name: &str, value: Option<bool>) {
    json::key(output, name);
    match value {
        Some(value) => output.push_str(if value { "true" } else { "false" }),
        None => output.push_str("null"),
    }
}
