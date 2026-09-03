//! Public poller diagnostics.

use crate::{Key, Poll};

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
