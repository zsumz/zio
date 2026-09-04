//! Stable NDJSON encoding for kqueue skew evidence.

use super::{
    config::Row,
    model::{Measurement, Outcome, Resources},
};
use crate::benchmark::{json, metadata::Metadata};

pub(super) fn encode(
    metadata: &Metadata,
    run_id: &str,
    row: Row,
    resources: Resources,
    outcome: &Outcome,
) -> String {
    let mut output = String::with_capacity(2_048);
    output.push('{');
    json::field_string(&mut output, "schema", "zio.kqueue-skew.v1", true);
    json::field_string(&mut output, "zio_version", metadata.qualify_version, true);
    json::field_string(
        &mut output,
        "status",
        match outcome {
            Outcome::Passed { .. } => "passed",
            Outcome::Unsupported { .. } => "unsupported",
            Outcome::Failed(_) => "failed",
        },
        true,
    );
    json::field_string(&mut output, "run_id", run_id, true);
    context(&mut output, metadata);
    parameters(&mut output, row);
    resource_limit(&mut output, resources);
    match outcome {
        Outcome::Passed { level, one_shot } => evidence_fields(&mut output, *level, *one_shot),
        Outcome::Unsupported { code, reason } => {
            json::field_string(&mut output, "reason_code", code, true);
            json::field_string(&mut output, "reason", reason, false);
        }
        Outcome::Failed(error) => {
            json::field_string(&mut output, "reason_code", "measurement_failed", true);
            json::field_string(&mut output, "reason", error, false);
        }
    }
    output.push('}');
    output
}

fn context(output: &mut String, metadata: &Metadata) {
    json::key(output, "git");
    output.push('{');
    json::field_string(output, "sha", &metadata.git_sha, true);
    json::field_string(output, "sha_source", metadata.git_sha_source, true);
    json::key(output, "dirty");
    match metadata.git_dirty {
        Some(value) => output.push_str(if value { "true" } else { "false" }),
        None => output.push_str("null"),
    }
    output.push_str("},");
    json::key(output, "host");
    output.push('{');
    json::field_string(output, "os", metadata.os, true);
    json::field_string(output, "os_version", &metadata.os_version, true);
    json::field_string(output, "arch", metadata.arch, true);
    json::field_string(output, "cpu", &metadata.cpu, true);
    json::field_string(output, "rustc", &metadata.rustc, true);
    json::field_string(output, "build_profile", metadata.build_profile, true);
    json::field_string(output, "rustflags", &metadata.rustflags, false);
    output.push_str("},");
    json::field_number(output, "recorded_unix_ms", metadata.recorded_unix_ms, true);
}

fn parameters(output: &mut String, row: Row) {
    json::key(output, "parameters");
    output.push('{');
    json::field_number(output, "registrations", row.registrations, true);
    json::field_number(output, "event_capacity", row.event_capacity, true);
    json::field_number(output, "ready_registrations", row.ready, true);
    json::field_string(output, "ready_fraction", row.ready_label, false);
    output.push_str("},");
}

fn resource_limit(output: &mut String, resources: Resources) {
    json::key(output, "resource_limit");
    output.push('{');
    optional_number(output, "open_fds", resources.open_fds);
    optional_number(output, "soft_fd_limit", resources.soft_fd_limit);
    json::field_string(
        output,
        "fd_limit_source",
        resources.fd_limit_source.unwrap_or("unavailable"),
        true,
    );
    json::field_number(
        output,
        "required_additional_fds",
        resources.required_additional_fds,
        false,
    );
    output.push_str("},");
}

fn evidence_fields(output: &mut String, level: Measurement, one_shot: Measurement) {
    json::key(output, "level");
    measurement(output, level);
    output.push(',');
    json::key(output, "one_shot");
    measurement(output, one_shot);
}

fn measurement(output: &mut String, value: Measurement) {
    output.push('{');
    json::field_number(output, "elapsed_ns", value.elapsed_ns, true);
    json::field_number(output, "waits_to_complete_cycle", value.waits, true);
    json::field_number(
        output,
        "raw_native_events_returned",
        value.native_observations,
        true,
    );
    json::field_number(
        output,
        "logical_events_delivered",
        value.logical_events,
        true,
    );
    json::field_number(
        output,
        "unique_registrations_delivered",
        value.unique_registrations,
        true,
    );
    json::field_number(
        output,
        "ns_per_logical_event",
        ratio(value.elapsed_ns, value.logical_events),
        true,
    );
    json::field_number(output, "disarm_submissions", value.disarm_submissions, true);
    json::field_number(
        output,
        "disarmed_registrations",
        value.disarmed_registrations,
        true,
    );
    json::field_number(
        output,
        "disarm_submission_elapsed_ns",
        value.disarm_elapsed_ns,
        true,
    );
    json::field_number(
        output,
        "disarm_ns_per_registration",
        ratio(value.disarm_elapsed_ns, value.disarmed_registrations),
        true,
    );
    json::key(output, "retained_heap");
    output.push('{');
    json::field_number(
        output,
        "allocation_count_current",
        value.retained_memory.allocation_count,
        true,
    );
    json::field_number(output, "bytes_current", value.retained_memory.bytes, true);
    json::field_number(
        output,
        "bytes_peak",
        value.retained_memory.peak_bytes,
        false,
    );
    output.push_str("}}");
}

fn ratio(numerator: u128, denominator: u64) -> u128 {
    if denominator == 0 {
        0
    } else {
        numerator / u128::from(denominator)
    }
}

fn optional_number(output: &mut String, name: &str, value: Option<u64>) {
    json::key(output, name);
    match value {
        Some(value) => output.push_str(&value.to_string()),
        None => output.push_str("null"),
    }
    output.push(',');
}
