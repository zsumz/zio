//! Safe epoll and eventfd backend composition.

use std::{io, os::fd::BorrowedFd, sync::Arc};

use crate::{
    ArmState, Interest, Mode, Readiness, Wait,
    error::{CommitStatus, Operation},
};

use super::super::{
    event::RawEvent,
    failure::{MutationFailure, SetupFailure},
};
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
    pub(crate) fn event(&self, index: usize, observed: usize) -> Option<RawEvent> {
        let event = self.raw.event(index, observed)?;
        if event.token() == 0 {
            Some(RawEvent::control())
        } else {
            Some(RawEvent::resource(
                event.token(),
                -1,
                from_epoll_flags(event.flags()),
            ))
        }
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
    wake: Arc<Wake>,
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
        epoll
            .add(wake.raw.as_fd(), 0, libc::EPOLLIN.cast_unsigned())
            .map_err(|source| SetupFailure::new(Operation::RegisterWaker, source))?;
        Ok((
            Self {
                epoll,
                wake: Arc::clone(&wake),
            },
            wake,
        ))
    }

    pub(crate) fn register(
        &self,
        source: BorrowedFd<'_>,
        token: u64,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), MutationFailure> {
        let flags = epoll_flags(token, interest, mode)?;
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
        let flags = epoll_flags(token, desired_interest, desired_mode)?;
        self.epoll.modify(source, token, flags).map_err(not_applied)
    }

    pub(crate) fn delete(
        &self,
        source: BorrowedFd<'_>,
        _interest: Interest,
    ) -> Result<(), MutationFailure> {
        self.epoll.delete(source).map_err(not_applied)
    }

    pub(crate) fn wait(&self, batch: &mut RawBatch, wait: Wait) -> io::Result<usize> {
        self.epoll.wait(&mut batch.raw, epoll_timeout(wait))
    }

    pub(crate) fn acknowledge_wake(&self) -> io::Result<()> {
        self.wake.raw.drain()
    }
}

fn epoll_flags(token: u64, interest: Interest, mode: Mode) -> Result<u32, MutationFailure> {
    if token == 0 || interest.is_empty() {
        return Err(invalid_mutation(
            "registration token and interest must be nonzero",
        ));
    }
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
    Ok(flags)
}

fn from_epoll_flags(flags: u32) -> Readiness {
    let contains = |flag: libc::c_int| flags & flag.cast_unsigned() != 0;
    let mut readiness = Readiness::EMPTY;
    if contains(libc::EPOLLIN) || contains(libc::EPOLLPRI) {
        readiness = readiness.union(Readiness::READABLE);
    }
    if contains(libc::EPOLLOUT) {
        readiness = readiness.union(Readiness::WRITABLE);
    }
    if contains(libc::EPOLLRDHUP) || contains(libc::EPOLLHUP) {
        readiness = readiness.union(Readiness::READ_CLOSED);
    }
    if contains(libc::EPOLLHUP) {
        readiness = readiness.union(Readiness::WRITE_CLOSED);
    }
    if contains(libc::EPOLLERR) {
        readiness = readiness.union(Readiness::ERROR);
    }
    readiness
}

fn epoll_timeout(wait: Wait) -> libc::c_int {
    match wait.timeout() {
        None => -1,
        Some(duration) => {
            let milliseconds = duration.as_millis().min(libc::c_int::MAX as u128);
            libc::c_int::try_from(milliseconds).unwrap_or(libc::c_int::MAX)
        }
    }
}

fn not_applied(source: io::Error) -> MutationFailure {
    MutationFailure::new(CommitStatus::NotApplied, source)
}

fn invalid_mutation(message: &'static str) -> MutationFailure {
    not_applied(io::Error::new(io::ErrorKind::InvalidInput, message))
}
