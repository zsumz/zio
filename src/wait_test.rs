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

#[test]
fn standard_timeouts_convert_without_losing_semantics() {
    let duration = Duration::from_millis(7);

    assert_eq!(Wait::from(duration), Wait::For(duration));
    assert_eq!(Wait::from(Some(duration)), Wait::For(duration));
    assert_eq!(Wait::from(None), Wait::Forever);
}
