//! Shared integration-test assertions.

use std::{
    io,
    os::fd::{AsRawFd, BorrowedFd},
};

use zio::{RecoveryFailure, WaitReport};

#[allow(
    dead_code,
    unsafe_code,
    reason = "shared read-only fcntl helper verifies descriptor contracts"
)]
pub(crate) fn descriptor_flags(descriptor: BorrowedFd<'_>) -> io::Result<i32> {
    // SAFETY: `descriptor` remains open for this read-only synchronous call.
    let flags = unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_GETFD) };
    if flags < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(flags)
    }
}

#[allow(
    dead_code,
    reason = "shared across independent integration-test crates"
)]
pub(crate) fn require_no_recovery(report: WaitReport) -> Result<(), RecoveryFailure> {
    match report.into_recovery() {
        Some(failure) => Err(failure),
        None => Ok(()),
    }
}
