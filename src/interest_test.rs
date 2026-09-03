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
fn intersection_and_difference_match_membership() {
    const COMBINED: Interest = Interest::READABLE.union(Interest::WRITABLE);
    const READABLE: Interest = COMBINED.intersection(Interest::READABLE);
    const WRITABLE: Interest = COMBINED.difference(Interest::READABLE);

    assert!(COMBINED.intersects(Interest::WRITABLE));
    assert_eq!(READABLE, Interest::READABLE);
    assert_eq!(WRITABLE, Interest::WRITABLE);
    assert_eq!(COMBINED & Interest::READABLE, READABLE);
    assert_eq!(COMBINED - Interest::READABLE, WRITABLE);

    let mut assigned = COMBINED;
    assigned &= Interest::WRITABLE;
    assigned -= Interest::WRITABLE;
    assert_eq!(assigned, Interest::EMPTY);
}
