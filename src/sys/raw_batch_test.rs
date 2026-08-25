//! Fixed kqueue recovery-storage bounds.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use crate::{Interest, RegistrationId};

use super::raw_batch::RawBatch;

#[test]
fn disarm_storage_uses_the_smaller_configured_limit() {
    assert_disarm_limit(2, 5, 2);
    assert_disarm_limit(5, 2, 2);
}

fn assert_disarm_limit(events: usize, registrations: usize, expected: usize) {
    let batch = RawBatch::new(events, registrations);
    assert!(batch.is_some());
    let Some(mut batch) = batch else {
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
