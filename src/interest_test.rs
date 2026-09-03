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
