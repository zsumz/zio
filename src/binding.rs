//! Borrowed registration state used at native backend boundaries.
#![cfg_attr(
    not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "netbsd"
    )),
    allow(dead_code, reason = "matches the target-selected observation contract")
)]

use std::os::fd::{BorrowedFd, RawFd};

use crate::{Interest, Key, Mode, RegistrationId, RegistrationState};

/// Borrowed native state for one exact registration.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Binding<'a> {
    pub(crate) descriptor: BorrowedFd<'a>,
    pub(crate) interest: Interest,
    pub(crate) mode: Mode,
    pub(crate) state: RegistrationState,
}

/// Copyable state needed while translating one native observation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Observation {
    pub(crate) id: RegistrationId,
    pub(crate) descriptor: RawFd,
    pub(crate) key: Key,
    #[cfg(target_os = "linux")]
    pub(crate) mode: Mode,
}
