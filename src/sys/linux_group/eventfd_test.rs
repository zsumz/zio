//! Linux eventfd saturation recovery tests.

use std::io;

use super::eventfd::EventFd;

#[test]
fn saturated_counter_is_reset_and_retriggered() -> io::Result<()> {
    let event = EventFd::new()?;
    event.saturate_for_test()?;

    event.wake()?;

    assert_eq!(event.read_test_value()?, 1);
    Ok(())
}
