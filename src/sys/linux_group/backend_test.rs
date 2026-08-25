//! Linux backend timeout conversion tests.

use core::time::Duration;

use crate::{Readiness, Wait};

use super::backend::{epoll_test_flags, epoll_timeout, from_epoll_flags};

#[test]
fn epoll_readiness_truth_table_is_exhaustive() {
    let flags = epoll_test_flags();
    for selected in 0_u32..(1 << flags.len()) {
        let native = flags
            .iter()
            .enumerate()
            .filter(|(index, _)| selected & (1 << index) != 0)
            .fold(0, |combined, (_, flag)| combined | flag);

        assert_eq!(
            from_epoll_flags(native),
            expected_readiness(selected),
            "native flags {native:#x}",
        );
    }
}

#[test]
fn epoll_error_closure_inference_is_deliberately_narrow() {
    let [input, _, output, _, _, error, _] = epoll_test_flags();
    let cases = [
        (error, Readiness::ERROR),
        (input | error, Readiness::READABLE.union(Readiness::ERROR)),
        (output | error, Readiness::WRITABLE.union(Readiness::ERROR)),
    ];

    for (native, expected) in cases {
        assert_eq!(
            from_epoll_flags(native),
            expected,
            "native flags {native:#x}"
        );
    }
}

#[test]
fn epoll_combined_delivery_preserves_every_hint() {
    let [input, _, output, read_hangup, hangup, error, _] = epoll_test_flags();
    let native = input | output | read_hangup | hangup | error;
    let expected = Readiness::READABLE
        .union(Readiness::WRITABLE)
        .union(Readiness::READ_CLOSED)
        .union(Readiness::WRITE_CLOSED)
        .union(Readiness::ERROR);

    assert_eq!(from_epoll_flags(native), expected);
}

fn expected_readiness(selected: u32) -> Readiness {
    let includes = |index: u32| selected & (1_u32 << index) != 0;
    let mut readiness = Readiness::EMPTY;
    if includes(0) || includes(1) {
        readiness = readiness.union(Readiness::READABLE);
    }
    if includes(2) {
        readiness = readiness.union(Readiness::WRITABLE);
    }
    if includes(3) || includes(4) {
        readiness = readiness.union(Readiness::READ_CLOSED);
    }
    if includes(4) {
        readiness = readiness.union(Readiness::WRITE_CLOSED);
    }
    if includes(5) {
        readiness = readiness.union(Readiness::ERROR);
    }
    readiness
}

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
