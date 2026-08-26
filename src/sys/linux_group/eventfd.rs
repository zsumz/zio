//! Owned nonblocking eventfd wake syscall leaf.

#![allow(
    unsafe_code,
    reason = "reviewed eventfd FFI is confined to this syscall leaf"
)]

use std::{
    io,
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd},
};

/// One owned nonblocking eventfd counter.
#[derive(Debug)]
pub(super) struct EventFd {
    descriptor: OwnedFd,
}

enum WriteOutcome {
    Complete,
    Interrupted,
    WouldBlock,
}

enum ReadOutcome {
    Complete(libc::eventfd_t),
    Interrupted,
    WouldBlock,
}

impl EventFd {
    pub(super) fn new() -> io::Result<Self> {
        // SAFETY: valid initial value and flags return a fresh descriptor or
        // transfer no ownership on failure.
        let descriptor = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: ownership of the fresh descriptor is transferred once.
        let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor) };
        Ok(Self { descriptor })
    }

    pub(super) fn as_fd(&self) -> BorrowedFd<'_> {
        self.descriptor.as_fd()
    }

    pub(super) fn wake(&self) -> io::Result<()> {
        let value: libc::eventfd_t = 1;
        loop {
            match self.write_once(value)? {
                WriteOutcome::Complete => return Ok(()),
                WriteOutcome::Interrupted => {}
                // The counter reached its maximum. Reset and write again so
                // edge-triggered epoll observes a fresh notification.
                WriteOutcome::WouldBlock => self.drain()?,
            }
        }
    }

    pub(super) fn drain(&self) -> io::Result<()> {
        loop {
            match self.read_once()? {
                ReadOutcome::Complete(_drained) => return Ok(()),
                ReadOutcome::WouldBlock => return Ok(()),
                ReadOutcome::Interrupted => {}
            }
        }
    }

    #[cfg(test)]
    pub(super) fn saturate_for_test(&self) -> io::Result<()> {
        loop {
            match self.write_once(libc::eventfd_t::MAX - 1)? {
                WriteOutcome::Complete => return Ok(()),
                WriteOutcome::Interrupted => {}
                WriteOutcome::WouldBlock => {
                    return Err(io::Error::other("fresh eventfd was already saturated"));
                }
            }
        }
    }

    #[cfg(test)]
    pub(super) fn read_test_value(&self) -> io::Result<u64> {
        loop {
            match self.read_once()? {
                ReadOutcome::Complete(value) => return Ok(value),
                ReadOutcome::Interrupted => {}
                ReadOutcome::WouldBlock => {
                    return Err(io::Error::other("eventfd test counter was empty"));
                }
            }
        }
    }

    fn write_once(&self, value: libc::eventfd_t) -> io::Result<WriteOutcome> {
        // SAFETY: the kernel copies the initialized scalar and retains no pointer.
        let result = unsafe {
            libc::write(
                self.descriptor.as_raw_fd(),
                (&raw const value).cast(),
                core::mem::size_of_val(&value),
            )
        };
        if result == 8 {
            Ok(WriteOutcome::Complete)
        } else if result < 0 {
            classify_write_error(io::Error::last_os_error())
        } else {
            Err(io::Error::other("eventfd wake wrote a partial scalar"))
        }
    }

    fn read_once(&self) -> io::Result<ReadOutcome> {
        let mut value: libc::eventfd_t = 0;
        // SAFETY: `value` is writable storage for the exact scalar size.
        let result = unsafe {
            libc::read(
                self.descriptor.as_raw_fd(),
                (&raw mut value).cast(),
                core::mem::size_of_val(&value),
            )
        };
        if result == 8 {
            Ok(ReadOutcome::Complete(value))
        } else if result < 0 {
            classify_read_error(io::Error::last_os_error())
        } else {
            Err(io::Error::other("eventfd drain read a partial scalar"))
        }
    }
}

fn classify_write_error(error: io::Error) -> io::Result<WriteOutcome> {
    match error.kind() {
        io::ErrorKind::Interrupted => Ok(WriteOutcome::Interrupted),
        io::ErrorKind::WouldBlock => Ok(WriteOutcome::WouldBlock),
        _ => Err(error),
    }
}

fn classify_read_error(error: io::Error) -> io::Result<ReadOutcome> {
    match error.kind() {
        io::ErrorKind::Interrupted => Ok(ReadOutcome::Interrupted),
        io::ErrorKind::WouldBlock => Ok(ReadOutcome::WouldBlock),
        _ => Err(error),
    }
}
