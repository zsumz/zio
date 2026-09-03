//! Event representation regressions.

use super::Event;

#[cfg(target_pointer_width = "64")]
#[test]
fn resource_event_layout_stays_compact() {
    assert_eq!(core::mem::size_of::<Event>(), 32);
}
