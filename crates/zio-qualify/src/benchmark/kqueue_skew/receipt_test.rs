//! Kqueue skew receipt regressions.

use super::{
    config::MATRIX,
    model::{Measurement, Outcome, Resources, RetainedMemory},
    receipt::encode,
};
use crate::benchmark::metadata;

#[test]
fn unsupported_receipt_preserves_parameters_and_limits() {
    let receipt = encode(
        &metadata::fixture(),
        "01234567-89ab-4cde-8f01-23456789abcd",
        MATRIX[0],
        Resources {
            open_fds: Some(11),
            soft_fd_limit: Some(64),
            fd_limit_source: Some("fixture"),
            required_additional_fds: 100_008,
        },
        &Outcome::Unsupported {
            code: "insufficient_fd_limit",
            reason: "fixture".to_owned(),
        },
    );
    assert!(receipt.contains("\"schema\":\"zio.kqueue-skew.v1\""));
    assert!(receipt.contains("\"run_id\":\"01234567-89ab-4cde-8f01-23456789abcd\""));
    assert!(receipt.contains("\"registrations\":100000"));
    assert!(receipt.contains("\"event_capacity\":64"));
    assert!(receipt.contains("\"ready_registrations\":100"));
    assert!(receipt.contains("\"required_additional_fds\":100008"));
    assert!(receipt.contains("\"status\":\"unsupported\""));
}

#[test]
fn passed_receipt_names_every_required_metric() {
    let measurement = Measurement {
        elapsed_ns: 120,
        waits: 2,
        native_observations: 17,
        logical_events: 3,
        unique_registrations: 3,
        disarm_submissions: 2,
        disarmed_registrations: 3,
        disarm_elapsed_ns: 30,
        retained_memory: RetainedMemory {
            allocation_count: 7,
            bytes: 4_096,
            peak_bytes: 4_096,
        },
    };
    let receipt = encode(
        &metadata::fixture(),
        "01234567-89ab-4cde-8f01-23456789abcd",
        MATRIX[0],
        Resources {
            open_fds: Some(11),
            soft_fd_limit: Some(200_000),
            fd_limit_source: Some("fixture"),
            required_additional_fds: 100_008,
        },
        &Outcome::Passed {
            level: measurement,
            one_shot: measurement,
        },
    );

    for field in [
        "\"raw_native_events_returned\":17",
        "\"logical_events_delivered\":3",
        "\"unique_registrations_delivered\":3",
        "\"ns_per_logical_event\":40",
        "\"waits_to_complete_cycle\":2",
        "\"disarm_submissions\":2",
        "\"disarmed_registrations\":3",
        "\"disarm_submission_elapsed_ns\":30",
        "\"disarm_ns_per_registration\":10",
        "\"bytes_current\":4096",
    ] {
        assert!(receipt.contains(field), "missing {field}");
    }
    assert!(receipt.contains("\"status\":\"passed\""));
}
