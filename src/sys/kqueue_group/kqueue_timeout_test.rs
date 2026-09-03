//! Kqueue timeout conversion regressions.

use core::time::Duration;

use super::kqueue_timeout::into_timespec;

#[test]
fn timeouts_preserve_precision_and_clamp_seconds() {
    let tiny = into_timespec(Duration::from_nanos(1));
    assert_eq!((tiny.tv_sec, tiny.tv_nsec), (0, 1));

    let exact = into_timespec(Duration::new(7, 123));
    assert_eq!((exact.tv_sec, exact.tv_nsec), (7, 123));

    let maximum = into_timespec(Duration::MAX);
    assert_eq!(maximum.tv_sec, i64::MAX);
    assert_eq!(maximum.tv_nsec, 999_999_999);
}
