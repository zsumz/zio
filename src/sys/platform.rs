//! Concrete compile-time facade over the target-selected backend.

use std::{io, os::fd::BorrowedFd};

use crate::{
    ArmState, Events, Interest, Mode, RegistrationState, Wait,
    mutation::{DeleteRequest, ModifyRequest, MutationDriver, RegisterRequest},
};

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
        state: RegistrationState,
    ) -> Result<(), MutationFailure> {
        #[cfg(target_os = "linux")]
        {
            let _ = state;
            self.linux.delete(source, interest)
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            self.kqueue.delete(source, interest, state)
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            let _ = state;
            self.unsupported.delete(source, interest)
        }
    }

    pub(crate) fn wait(
        &self,
        batch: &mut RawBatch,
        events: &mut Events,
        wait: Wait,
    ) -> io::Result<usize> {
        #[cfg(target_os = "linux")]
        {
            self.linux.wait(&mut batch.linux, events, wait)
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            let _ = events;
            self.kqueue.wait(&mut batch.kqueue, wait)
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            let _ = events;
            self.unsupported.wait(&mut batch.unsupported, wait)
        }
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(crate) fn submit_disarms(&self, batch: &mut RawBatch) -> io::Result<()> {
        self.kqueue.submit_disarms(&mut batch.kqueue)
    }
}

impl MutationDriver for Backend {
    fn register(&mut self, request: RegisterRequest<'_>) -> Result<(), MutationFailure> {
        let _ = request.key;
        Backend::register(
            self,
            request.descriptor,
            request.registration.get(),
            request.interest,
            request.mode,
        )
    }

    fn modify(&mut self, request: ModifyRequest<'_>) -> Result<(), MutationFailure> {
        Backend::modify(
            self,
            request.descriptor,
            request.registration.get(),
            request.previous_interest,
            request.previous_mode,
            request.previous_arm,
            request.desired_interest,
            request.desired_mode,
        )
    }

    fn delete(&mut self, request: DeleteRequest<'_>) -> Result<(), MutationFailure> {
        let _ = request.registration;
        Backend::delete(self, request.descriptor, request.interest, request.state)
    }
}
