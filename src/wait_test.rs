//! Wait classification regressions.

use core::time::Duration;

use super::Wait;

#[test]
fn nonblocking_classification_includes_zero_duration() {
    assert!(Wait::NoBlock.is_nonblocking());
    assert!(Wait::For(Duration::ZERO).is_nonblocking());
    assert!(!Wait::For(Duration::from_nanos(1)).is_nonblocking());
    assert!(!Wait::Forever.is_nonblocking());
}
