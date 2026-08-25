//! Concrete compile-time facade over the target-selected backend.

use std::{io, os::fd::BorrowedFd};

use crate::{ArmState, Interest, Mode, Wait};

use super::{
    failure::{MutationFailure, SetupFailure},
    raw_batch::RawBatch,
    wake::Wake,
};

/// Target-selected selector with no dynamic dispatch.
#[derive(Debug)]
pub(crate) struct Backend {
    #[cfg(target_os = "linux")]
    linux: super::linux_group::Backend,
    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    kqueue: super::kqueue_group::Backend,
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "netbsd"
    )))]
    unsupported: super::unsupported::Backend,
}

impl Backend {
    pub(crate) fn raw_batch(events: usize, registrations: usize) -> Option<RawBatch> {
        RawBatch::new(events, registrations)
    }

    pub(crate) fn new() -> Result<(Self, Wake), SetupFailure> {
        #[cfg(target_os = "linux")]
        {
            super::linux_group::Backend::new()
                .map(|(linux, wake)| (Self { linux }, Wake { linux: wake }))
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            super::kqueue_group::Backend::new()
                .map(|(kqueue, wake)| (Self { kqueue }, Wake { kqueue: wake }))
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            super::unsupported::Backend::new()
                .map(|(unsupported, wake)| (Self { unsupported }, Wake { unsupported: wake }))
        }
    }

    pub(crate) fn register(
        &self,
        source: BorrowedFd<'_>,
        token: u64,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), MutationFailure> {
        #[cfg(target_os = "linux")]
        {
            self.linux.register(source, token, interest, mode)
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            self.kqueue.register(source, token, interest, mode)
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            self.unsupported.register(source, token, interest, mode)
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "mutation state is intentionally explicit"
    )]
    pub(crate) fn modify(
        &self,
        source: BorrowedFd<'_>,
        token: u64,
        previous_interest: Interest,
        previous_mode: Mode,
        previous_arm: ArmState,
        desired_interest: Interest,
        desired_mode: Mode,
    ) -> Result<(), MutationFailure> {
        #[cfg(target_os = "linux")]
        {
            self.linux.modify(
                source,
                token,
                previous_interest,
                previous_mode,
                previous_arm,
                desired_interest,
                desired_mode,
            )
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            self.kqueue.modify(
                source,
                token,
                previous_interest,
                previous_mode,
                previous_arm,
                desired_interest,
                desired_mode,
            )
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            self.unsupported.modify(
                source,
                token,
                previous_interest,
                previous_mode,
                previous_arm,
                desired_interest,
                desired_mode,
            )
        }
    }

    pub(crate) fn delete(
        &self,
        source: BorrowedFd<'_>,
        interest: Interest,
    ) -> Result<(), MutationFailure> {
        #[cfg(target_os = "linux")]
        {
            self.linux.delete(source, interest)
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            self.kqueue.delete(source, interest)
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            self.unsupported.delete(source, interest)
        }
    }

    pub(crate) fn wait(&self, batch: &mut RawBatch, wait: Wait) -> io::Result<usize> {
        #[cfg(target_os = "linux")]
        {
            self.linux.wait(&mut batch.linux, wait)
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            self.kqueue.wait(&mut batch.kqueue, wait)
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            self.unsupported.wait(&mut batch.unsupported, wait)
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn acknowledge_wake(&self) -> io::Result<()> {
        self.linux.acknowledge_wake()
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(crate) fn disarm(
        &self,
        descriptor: std::os::fd::RawFd,
        interest: Interest,
    ) -> Result<(), MutationFailure> {
        self.kqueue.disarm(descriptor, interest)
    }
}
