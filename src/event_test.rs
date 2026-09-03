//! Event representation regressions.

use super::{Event, Readiness};

#[cfg(target_pointer_width = "64")]
#[test]
fn resource_event_layout_stays_compact() {
    assert_eq!(core::mem::size_of::<Event>(), 32);
}

#[test]
fn readiness_union_operators_match_membership() {
    let combined = Readiness::READABLE | Readiness::READ_CLOSED;
    assert!(combined.is_readable());
    assert!(combined.is_read_closed());

    let mut assigned = Readiness::READABLE;
    assigned |= Readiness::READ_CLOSED;
    assert_eq!(assigned, combined);
}
