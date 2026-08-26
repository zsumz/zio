//! Live and retained descriptor receipt fields.

use super::{json, measure::FdProbe, record::Sample};

pub(crate) fn live_fds(output: &mut String, samples: &[Sample]) {
    json::key(output, "live_fds");
    output.push('{');
    let values: Option<Vec<_>> = samples
        .iter()
        .map(|sample| sample.captured.resources.live_fds)
        .collect();
    let Some(values) = values.filter(|values| !values.is_empty()) else {
        json::field_string(output, "status", "unavailable", true);
        json::field_string(
            output,
            "reason",
            "no complete live descriptor snapshots",
            false,
        );
        output.push('}');
        return;
    };
    json::field_string(output, "status", "available", true);
    array(
        output,
        "fixture_baseline",
        values.iter().map(|value| value.fixture_baseline),
    );
    array(
        output,
        "candidate_setup",
        values.iter().map(|value| value.candidate_setup),
    );
    array(output, "active", values.iter().map(|value| value.active));
    array(
        output,
        "post_cleanup",
        values.iter().map(|value| value.post_cleanup),
    );
    signed_array(
        output,
        "setup_delta",
        values.iter().filter_map(|value| value.setup_delta()),
    );
    signed_array(
        output,
        "active_delta",
        values.iter().filter_map(|value| value.active_delta()),
    );
    json::key(output, "post_cleanup_delta");
    json::array_i64(
        output,
        values.iter().filter_map(|value| value.cleanup_delta()),
    );
    output.push('}');
}

pub(crate) fn retained_fds(output: &mut String, probe: &FdProbe, samples: &[Sample]) {
    json::key(output, "retained_fds");
    output.push('{');
    if samples.is_empty() {
        json::field_string(output, "status", "unavailable", true);
        json::field_string(output, "reason", "no completed samples", false);
    } else if samples
        .iter()
        .all(|sample| sample.retained_fd_delta.is_some())
    {
        json::field_string(output, "status", "available", true);
        json::field_string(output, "path", probe.path().unwrap_or("unavailable"), true);
        json::key(output, "raw_delta");
        json::array_i64(
            output,
            samples.iter().filter_map(|sample| sample.retained_fd_delta),
        );
    } else {
        json::field_string(output, "status", "unavailable", true);
        json::field_string(
            output,
            "reason",
            probe
                .reason()
                .unwrap_or("descriptor directory became unreadable during sampling"),
            false,
        );
    }
    output.push('}');
}

fn array(output: &mut String, name: &str, values: impl Iterator<Item = usize>) {
    json::key(output, name);
    json::array_usize(output, values);
    output.push(',');
}

fn signed_array(output: &mut String, name: &str, values: impl Iterator<Item = i64>) {
    json::key(output, name);
    json::array_i64(output, values);
    output.push(',');
}
