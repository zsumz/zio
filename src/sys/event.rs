//! Normalized observations detached from syscall-owned storage.

#![cfg_attr(
    not(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd")),
    allow(dead_code, reason = "matches the target-selected raw-event contract")
)]

use crate::Readiness;

/// One normalized resource or administrative observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RawEvent {
    token: u64,
    descriptor: i32,
    readiness: Readiness,
    control: bool,
}

impl RawEvent {
    pub(crate) const fn resource(token: u64, descriptor: i32, readiness: Readiness) -> Self {
        Self {
            token,
            descriptor,
            readiness,
            control: false,
        }
    }

    pub(crate) const fn control() -> Self {
        Self {
            token: 0,
            descriptor: -1,
            readiness: Readiness::EMPTY,
            control: true,
        }
    }

    pub(crate) const fn token(self) -> u64 {
        self.token
    }

    pub(crate) const fn descriptor(self) -> i32 {
        self.descriptor
    }

    pub(crate) const fn readiness(self) -> Readiness {
        self.readiness
    }

    pub(crate) const fn is_control(self) -> bool {
        self.control
    }
}
