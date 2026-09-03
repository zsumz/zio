//! Fixed kqueue recovery-storage bounds.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use crate::{CapacityKind, CapacityReason, Error, Interest, RegistrationId};

use super::raw_batch::RawBatch;

#[test]
fn native_observations_cover_every_filter_while_recovery_is_delivery_bounded() {
    let batch = RawBatch::new(1, 2);
    assert!(batch.is_ok());
    let Ok(mut batch) = batch else {
        return;
    };

    assert_eq!(batch.native_event_capacity(), 5);
    assert!(
        batch
            .push_disarm(RegistrationId::new(1), 1, Interest::READABLE)
            .is_some()
    );
    assert!(
        batch
            .push_disarm(RegistrationId::new(2), 2, Interest::READABLE)
            .is_none()
    );
}

#[test]
fn disarm_storage_uses_the_smaller_configured_limit() {
    assert_disarm_limit(2, 5, 2);
    assert_disarm_limit(5, 2, 2);
}

fn assert_disarm_limit(events: usize, registrations: usize, expected: usize) {
    let batch = RawBatch::new(events, registrations);
    assert!(batch.is_ok());
    let Ok(mut batch) = batch else {
        return;
    };

    for _ in 0..expected {
        assert!(
            batch
                .push_disarm(RegistrationId::new(1), 1, Interest::READABLE)
                .is_some()
        );
    }
    assert!(
        batch
            .push_disarm(RegistrationId::new(1), 1, Interest::READABLE)
            .is_none()
    );
}

#[test]
#[cfg(target_pointer_width = "64")]
fn oversized_native_observation_capacity_reports_registration_limit() {
    assert!(matches!(
        RawBatch::new(1, usize::MAX),
        Err(Error::Capacity {
            kind: CapacityKind::Registration,
            limit: usize::MAX,
            reason: CapacityReason::BackendLimit,
        })
    ));
}
