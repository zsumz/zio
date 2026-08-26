//! Safe epoll and eventfd backend composition.

use std::{io, os::fd::BorrowedFd, sync::Arc};

use crate::{
    ArmState, Error, Events, Interest, Key, Mode, Readiness, Wait,
    error::{CommitStatus, Operation},
};

use super::super::failure::{MutationFailure, SetupFailure};
use super::{
    epoll::{Epoll, EpollBatch},
    eventfd::EventFd,
};

/// Fixed Linux kernel-event storage.
#[derive(Debug)]
pub(crate) struct RawBatch {
    raw: EpollBatch,
}

impl RawBatch {
    #[inline]
    pub(crate) fn translate<F>(
        &mut self,
        events: &mut Events,
        observed: usize,
        wake_key: Option<Key>,
        classify: F,
    ) -> Result<(), Error>
    where
        F: FnMut(u64) -> Result<Option<Key>, Error>,
    {
        self.raw.translate(events, observed, wake_key, classify)
    }
}

/// Clone-shared eventfd wake handle.
#[derive(Debug)]
pub(crate) struct Wake {
    raw: EventFd,
}

impl Wake {
    pub(crate) fn wake(&self) -> io::Result<()> {
        self.raw.wake()
    }
}

/// Linux selector and its reserved wake source.
#[derive(Debug)]
pub(crate) struct Backend {
    epoll: Epoll,
}

impl Backend {
    pub(crate) fn raw_batch(capacity: usize) -> Option<RawBatch> {
        EpollBatch::new(capacity).map(|raw| RawBatch { raw })
    }

    pub(crate) fn new() -> Result<(Self, Arc<Wake>), SetupFailure> {
        let epoll =
            Epoll::new().map_err(|source| SetupFailure::new(Operation::CreatePoller, source))?;
        let wake = Arc::new(Wake {
            raw: EventFd::new()
                .map_err(|source| SetupFailure::new(Operation::CreateWaker, source))?,
        });
        // Edge delivery makes consuming an epoll observation the logical
        // acknowledgement. A later eventfd write queues a fresh edge without
        // a read syscall on the normal wake path.
        let wake_flags = libc::EPOLLIN.cast_unsigned() | libc::EPOLLET.cast_unsigned();
        epoll
            .add(wake.raw.as_fd(), 0, wake_flags)
            .map_err(|source| SetupFailure::new(Operation::RegisterWaker, source))?;
        Ok((Self { epoll }, wake))
    }

    pub(crate) fn register(
        &self,
        source: BorrowedFd<'_>,
        token: u64,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), MutationFailure> {
        let flags = epoll_flags(token, interest, mode);
        self.epoll.add(source, token, flags).map_err(not_applied)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "mutation state is intentionally explicit"
    )]
    pub(crate) fn modify(
        &self,
        source: BorrowedFd<'_>,
        token: u64,
        _previous_interest: Interest,
        _previous_mode: Mode,
        _previous_arm: ArmState,
        desired_interest: Interest,
        desired_mode: Mode,
    ) -> Result<(), MutationFailure> {
        let flags = epoll_flags(token, desired_interest, desired_mode);
        self.epoll.modify(source, token, flags).map_err(not_applied)
    }

    pub(crate) fn delete(
        &self,
        source: BorrowedFd<'_>,
        _interest: Interest,
    ) -> Result<(), MutationFailure> {
        self.epoll.delete(source).map_err(not_applied)
    }

    pub(crate) fn wait(
        &self,
        batch: &mut RawBatch,
        events: &mut Events,
        wait: Wait,
    ) -> io::Result<usize> {
        self.epoll.wait(&mut batch.raw, events, epoll_timeout(wait))
    }
}

#[inline]
fn epoll_flags(token: u64, interest: Interest, mode: Mode) -> u32 {
    debug_assert_ne!(token, 0);
    debug_assert!(!interest.is_empty());
    let mut flags = libc::EPOLLRDHUP.cast_unsigned();
    if interest.is_readable() {
        flags |= libc::EPOLLIN.cast_unsigned();
    }
    if interest.is_writable() {
        flags |= libc::EPOLLOUT.cast_unsigned();
    }
    if mode == Mode::OneShot {
        flags |= libc::EPOLLONESHOT.cast_unsigned();
    }
    flags
}

#[inline]
pub(super) fn from_epoll_flags(flags: u32) -> Readiness {
    let contains = |flag: libc::c_int| flags & flag.cast_unsigned() != 0;
    let mut readiness = Readiness::EMPTY;
    if contains(libc::EPOLLIN) || contains(libc::EPOLLPRI) {
        readiness = readiness.union(Readiness::READABLE);
    }
    if contains(libc::EPOLLOUT) {
        readiness = readiness.union(Readiness::WRITABLE);
    }
    let hung_up = contains(libc::EPOLLHUP);
    let errored = contains(libc::EPOLLERR);
    if contains(libc::EPOLLRDHUP) || hung_up {
        readiness = readiness.union(Readiness::READ_CLOSED);
    }
    if hung_up {
        readiness = readiness.union(Readiness::WRITE_CLOSED);
    }
    if errored {
        readiness = readiness.union(Readiness::ERROR);
    }
    readiness
}

#[cfg(test)]
pub(super) const fn epoll_test_flags() -> [u32; 7] {
    [
        libc::EPOLLIN.cast_unsigned(),
        libc::EPOLLPRI.cast_unsigned(),
        libc::EPOLLOUT.cast_unsigned(),
        libc::EPOLLRDHUP.cast_unsigned(),
        libc::EPOLLHUP.cast_unsigned(),
        libc::EPOLLERR.cast_unsigned(),
        libc::EPOLLONESHOT.cast_unsigned(),
    ]
}

pub(super) fn epoll_timeout(wait: Wait) -> libc::c_int {
    match wait.timeout() {
        None => -1,
        Some(duration) => {
            const NANOS_PER_MILLISECOND: u128 = 1_000_000;

            let milliseconds = duration
                .as_nanos()
                .div_ceil(NANOS_PER_MILLISECOND)
                .min(libc::c_int::MAX as u128);
            libc::c_int::try_from(milliseconds).unwrap_or(libc::c_int::MAX)
        }
    }
}

fn not_applied(source: io::Error) -> MutationFailure {
    MutationFailure::new(CommitStatus::NotApplied, source)
}
