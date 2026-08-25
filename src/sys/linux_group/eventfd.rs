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
            // SAFETY: the kernel copies the initialized eight-byte scalar during
            // this call and does not retain its pointer.
            let result = unsafe {
                libc::write(
                    self.descriptor.as_raw_fd(),
                    (&raw const value).cast(),
                    core::mem::size_of_val(&value),
                )
            };
            if result == 8 {
                return Ok(());
            }
            if result >= 0 {
                return Err(io::Error::other("eventfd wake wrote a partial scalar"));
            }
            let error = io::Error::last_os_error();
            match error.kind() {
                io::ErrorKind::Interrupted => {}
                io::ErrorKind::WouldBlock => return Ok(()),
                _ => return Err(error),
            }
        }
    }

    pub(super) fn drain(&self) -> io::Result<()> {
        let mut value: libc::eventfd_t = 0;
        loop {
            // SAFETY: `value` is uniquely borrowed writable storage for the exact
            // scalar size during this call.
            let result = unsafe {
                libc::read(
                    self.descriptor.as_raw_fd(),
                    (&raw mut value).cast(),
                    core::mem::size_of_val(&value),
                )
            };
            if result == 8 {
                return Ok(());
            }
            if result >= 0 {
                return Err(io::Error::other("eventfd drain read a partial scalar"));
            }
            let error = io::Error::last_os_error();
            match error.kind() {
                io::ErrorKind::Interrupted => {}
                io::ErrorKind::WouldBlock => return Ok(()),
                _ => return Err(error),
            }
        }
    }
}
