//! Linux backend timeout conversion tests.

use core::time::Duration;

use crate::Wait;

use super::backend::epoll_timeout;

#[test]
fn zero_timeouts_remain_nonblocking() {
    assert_eq!(epoll_timeout(Wait::NoBlock), 0);
    assert_eq!(epoll_timeout(Wait::For(Duration::ZERO)), 0);
}

#[test]
fn one_nanosecond_rounds_up_to_one_millisecond() {
    assert_eq!(epoll_timeout(Wait::For(Duration::from_nanos(1))), 1);
}

#[test]
fn positive_sub_millisecond_timeout_rounds_up() {
    assert_eq!(epoll_timeout(Wait::For(Duration::from_micros(999))), 1);
}

#[test]
fn exact_millisecond_is_unchanged() {
    assert_eq!(epoll_timeout(Wait::For(Duration::from_millis(1))), 1);
}

#[test]
fn large_timeout_clamps_after_rounding() {
    let duration = Duration::from_millis(i32::MAX as u64).saturating_add(Duration::from_nanos(1));

    assert_eq!(epoll_timeout(Wait::For(duration)), i32::MAX);
}

#[test]
fn forever_uses_epoll_indefinite_timeout() {
    assert_eq!(epoll_timeout(Wait::Forever), -1);
}
