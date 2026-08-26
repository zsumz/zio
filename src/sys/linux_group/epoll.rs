//! Fixed epoll storage and reviewed syscall leaf.

#![allow(
    unsafe_code,
    reason = "reviewed epoll FFI is confined to this syscall leaf"
)]

use std::{
    io,
    mem::MaybeUninit,
    os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd},
};

/// One copied epoll observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EpollEvent {
    flags: u32,
    token: u64,
}

impl EpollEvent {
    pub(super) const fn flags(self) -> u32 {
        self.flags
    }

    pub(super) const fn token(self) -> u64 {
        self.token
    }
}

/// Fixed storage initialized only by `epoll_wait`.
pub(super) struct EpollBatch {
    events: Box<[MaybeUninit<libc::epoll_event>]>,
    max_events: libc::c_int,
    observed: usize,
}

impl EpollBatch {
    pub(super) fn new(capacity: usize) -> Option<Self> {
        let max_events = libc::c_int::try_from(capacity)
            .ok()
            .filter(|count| *count > 0)?;
        let mut events = Vec::new();
        events.try_reserve_exact(capacity).ok()?;
        events.resize_with(capacity, MaybeUninit::uninit);
        Some(Self {
            events: events.into_boxed_slice(),
            max_events,
            observed: 0,
        })
    }

    pub(super) fn event(&self, index: usize, observed: usize) -> Option<EpollEvent> {
        if observed != self.observed || index >= observed {
            return None;
        }
        let event = self.events.get(index)?;
        // SAFETY: `wait` records exactly the prefix initialized by the most
        // recent successful `epoll_wait`, and both bounds were checked above.
        let event = unsafe { event.assume_init_ref() };
        Some(EpollEvent {
            flags: event.events,
            token: event.u64,
        })
    }
}

impl std::fmt::Debug for EpollBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EpollBatch")
            .field("capacity", &self.events.len())
            .finish_non_exhaustive()
    }
}

/// One owned epoll instance.
#[derive(Debug)]
pub(super) struct Epoll {
    descriptor: OwnedFd,
}

impl Epoll {
    pub(super) fn new() -> io::Result<Self> {
        // SAFETY: the valid flag either returns a fresh descriptor or failure.
        let descriptor = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the successful call transferred one fresh descriptor.
        let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor) };
        Ok(Self { descriptor })
    }

    pub(super) fn add(&self, source: BorrowedFd<'_>, token: u64, flags: u32) -> io::Result<()> {
        self.control(libc::EPOLL_CTL_ADD, source, token, flags)
    }

    pub(super) fn modify(&self, source: BorrowedFd<'_>, token: u64, flags: u32) -> io::Result<()> {
        self.control(libc::EPOLL_CTL_MOD, source, token, flags)
    }

    pub(super) fn delete(&self, source: BorrowedFd<'_>) -> io::Result<()> {
        // SAFETY: both descriptors are live; Linux ignores the event pointer
        // for EPOLL_CTL_DEL.
        let result = unsafe {
            libc::epoll_ctl(
                self.descriptor.as_raw_fd(),
                libc::EPOLL_CTL_DEL,
                source.as_raw_fd(),
                core::ptr::null_mut(),
            )
        };
        syscall_result(result)
    }

    pub(super) fn wait(&self, batch: &mut EpollBatch, timeout: libc::c_int) -> io::Result<usize> {
        batch.observed = 0;
        // SAFETY: the batch owns `max_events` properly aligned writable slots.
        // `epoll_wait` initializes the returned prefix and retains no pointer.
        let observed = unsafe {
            libc::epoll_wait(
                self.descriptor.as_raw_fd(),
                batch.events.as_mut_ptr().cast(),
                batch.max_events,
                timeout,
            )
        };
        if observed < 0 {
            Err(io::Error::last_os_error())
        } else {
            let observed = usize::try_from(observed)
                .map_err(|_| io::Error::other("epoll returned an invalid event count"))?;
            batch.observed = observed;
            Ok(observed)
        }
    }

    fn control(
        &self,
        operation: libc::c_int,
        source: BorrowedFd<'_>,
        token: u64,
        flags: u32,
    ) -> io::Result<()> {
        let mut event = libc::epoll_event {
            events: flags,
            u64: token,
        };
        // SAFETY: both descriptors and the initialized event remain live for
        // the synchronous syscall.
        let result = unsafe {
            libc::epoll_ctl(
                self.descriptor.as_raw_fd(),
                operation,
                source.as_raw_fd(),
                &raw mut event,
            )
        };
        syscall_result(result)
    }
}

fn syscall_result(result: libc::c_int) -> io::Result<()> {
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
