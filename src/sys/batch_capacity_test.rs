//! Native-batch capacity classification regressions.

use crate::{CapacityKind, CapacityReason, Error};

use super::batch_capacity::KqueueCapacity;

#[test]
fn storage_failures_name_the_dominant_capacity() -> Result<(), Error> {
    let event_dominant = KqueueCapacity::new(4, 5)?;
    assert_capacity(&event_dominant.arena_error(), CapacityKind::Event, 4);
    assert_capacity(&event_dominant.recovery_error(), CapacityKind::Event, 4);

    let registration_dominant = KqueueCapacity::new(2, 5)?;
    assert_capacity(
        &registration_dominant.arena_error(),
        CapacityKind::Registration,
        5,
    );
    Ok(())
}

#[test]
#[cfg(target_pointer_width = "64")]
fn arithmetic_overflow_names_registration_capacity() {
    assert!(matches!(
        KqueueCapacity::new(1, usize::MAX),
        Err(Error::Capacity {
            kind: CapacityKind::Registration,
            limit: usize::MAX,
            reason: CapacityReason::BackendLimit,
        })
    ));
}

fn assert_capacity(error: &Error, expected_kind: CapacityKind, expected_limit: usize) {
    assert!(matches!(
        error,
        Error::Capacity {
            kind,
            limit,
            reason: CapacityReason::StorageUnavailable,
        } if *kind == expected_kind && *limit == expected_limit
    ));
}
