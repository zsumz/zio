//! Kqueue wait timeout conversion.

use std::time::Duration;

pub(super) fn into_timespec(duration: Duration) -> libc::timespec {
    let seconds = duration.as_secs().min(libc::time_t::MAX as u64);
    let tv_sec = libc::time_t::try_from(seconds).unwrap_or(libc::time_t::MAX);
    libc::timespec {
        tv_sec,
        tv_nsec: libc::c_long::from(duration.subsec_nanos()),
    }
}
