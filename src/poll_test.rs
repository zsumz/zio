//! Public poller diagnostics.

use crate::{CapacityKind, CapacityReason, Error, Key, Poll};

#[test]
fn zero_capacities_report_their_kind() {
    assert!(matches!(
        Poll::with_capacity(0, 1),
        Err(Error::Capacity {
            kind: CapacityKind::Event,
            limit: 0,
            reason: CapacityReason::Zero,
        })
    ));
    assert!(matches!(
        Poll::with_capacity(1, 0),
        Err(Error::Capacity {
            kind: CapacityKind::Registration,
            limit: 0,
            reason: CapacityReason::Zero,
        })
    ));
}

#[test]
fn debug_output_is_backend_neutral() -> Result<(), crate::Error> {
    let mut poll = Poll::with_capacity(3, 5)?;

    assert_eq!(
        format!("{poll:?}"),
        concat!(
            "Poll { event_capacity: 3, registration_capacity: 5, ",
            "registration_count: 0, remaining_registration_capacity: 5, ",
            "wake_key: None, .. }"
        )
    );
    let waker = poll.waker(Key::new(7))?;
    assert_eq!(format!("{waker:?}"), "Waker { key: Key(7), .. }");
    Ok(())
}
