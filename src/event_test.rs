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

#[test]
fn readiness_intersection_and_difference_match_membership() {
    const COMBINED: Readiness = Readiness::READABLE.union(Readiness::READ_CLOSED);
    const READABLE: Readiness = COMBINED.intersection(Readiness::READABLE);
    const CLOSED: Readiness = COMBINED.difference(Readiness::READABLE);

    assert!(COMBINED.intersects(Readiness::READ_CLOSED));
    assert_eq!(READABLE, Readiness::READABLE);
    assert_eq!(CLOSED, Readiness::READ_CLOSED);
    assert_eq!(COMBINED & Readiness::READABLE, READABLE);
    assert_eq!(COMBINED - Readiness::READABLE, CLOSED);

    let mut assigned = COMBINED;
    assigned &= Readiness::READ_CLOSED;
    assigned -= Readiness::READ_CLOSED;
    assert_eq!(assigned, Readiness::EMPTY);
}

#[test]
fn readiness_debug_uses_symbolic_flag_names() {
    assert_eq!(format!("{:?}", Readiness::EMPTY), "EMPTY");
    assert_eq!(
        format!(
            "{:?}",
            Readiness::READABLE | Readiness::READ_CLOSED | Readiness::ERROR
        ),
        "READABLE | READ_CLOSED | ERROR"
    );
}
