//! Interest flag operator regressions.

use super::Interest;

#[test]
fn union_operators_match_membership() {
    let combined = Interest::READABLE | Interest::WRITABLE;
    assert!(combined.is_readable());
    assert!(combined.is_writable());

    let mut assigned = Interest::READABLE;
    assigned |= Interest::WRITABLE;
    assert_eq!(assigned, combined);
}

#[test]
fn set_operations_match_membership() {
    const COMBINED: Interest = Interest::READABLE.union(Interest::WRITABLE);
    const READABLE: Interest = COMBINED.intersection(Interest::READABLE);
    const WRITABLE: Interest = COMBINED.difference(Interest::READABLE);

    assert!(COMBINED.intersects(Interest::WRITABLE));
    assert_eq!(READABLE, Interest::READABLE);
    assert_eq!(WRITABLE, Interest::WRITABLE);
    assert_eq!(COMBINED & Interest::READABLE, READABLE);
    assert_eq!(COMBINED - Interest::READABLE, WRITABLE);
    assert_eq!(COMBINED ^ Interest::READABLE, WRITABLE);
    assert_eq!(COMBINED.symmetric_difference(Interest::WRITABLE), READABLE);
    assert_eq!(Interest::READABLE.complement(), WRITABLE);
    assert_eq!(!Interest::READABLE, WRITABLE);
    assert_eq!(!Interest::EMPTY, Interest::ALL);

    let mut assigned = COMBINED;
    assigned &= Interest::WRITABLE;
    assigned ^= Interest::READABLE;
    assert_eq!(assigned, COMBINED);
    assigned -= Interest::WRITABLE;
    assert_eq!(assigned, Interest::READABLE);
}

#[test]
fn all_contains_every_supported_interest() {
    assert_eq!(Interest::ALL, Interest::READABLE | Interest::WRITABLE);
}

#[test]
fn debug_uses_symbolic_flag_names() {
    assert_eq!(format!("{:?}", Interest::EMPTY), "EMPTY");
    assert_eq!(
        format!("{:?}", Interest::READABLE | Interest::WRITABLE),
        "READABLE | WRITABLE"
    );
}
