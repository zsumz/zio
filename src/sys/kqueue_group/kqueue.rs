//! Owned kqueue descriptor and reviewed syscall leaf.

#![allow(
    unsafe_code,
    reason = "reviewed kqueue FFI is confined to this syscall leaf"
)]

#[cfg(not(target_pointer_width = "64"))]
const _: [(); 8] = [(); core::mem::size_of::<usize>()];

use std::{
    io, mem,
    mem::MaybeUninit,
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    ptr,
    time::Duration,
};

use super::{
    kqueue_arena::{KqueueArena, StagedChanges},
    kqueue_change::{Change, RawKevent},
    kqueue_codec::{
        classify_apply_error, decode_filter, decode_receipt, has_flag,
        target_applies_interrupted_changes,
    },
    kqueue_disarm::{FilterApply, NativeApply},
    kqueue_timeout::into_timespec,
};

#[derive(Debug)]
pub(super) struct KeventBatch {
    pub(super) arena: KqueueArena<libc::kevent>,
    pub(super) observed: usize,
    pub(super) receipts: usize,
}

impl KeventBatch {
    pub(super) fn new(event_capacity: usize, change_capacity: usize) -> Option<Self> {
        KqueueArena::new(event_capacity, change_capacity).map(|arena| Self {
            arena,
            observed: 0,
            receipts: 0,
        })
    }

    pub(super) fn event(&self, index: usize, observed: usize) -> Option<RawKevent> {
        if observed != self.observed || index >= observed {
            return None;
        }
        let event = self.arena.event_slot(index)?;
        // SAFETY: `wait` records the exact kernel-written prefix.
        let event = initialized_event(event);
        Some(RawKevent::new(
            RawFd::try_from(event.ident).ok()?,
            decode_filter(i64::from(event.filter)),
            event.udata as usize as u64,
            has_flag(u64::from(event.flags), u64::from(libc::EV_EOF)),
            has_flag(u64::from(event.flags), u64::from(libc::EV_ERROR)),
            event.fflags,
        ))
    }

    pub(super) fn receipt(&self, index: usize, returned: usize, expected: Change) -> FilterApply {
        let event = (returned == self.receipts && index < returned)
            .then(|| self.arena.receipt_slot(index))
            .flatten()
            .map(initialized_event);
        match decode_receipt(event, expected) {
            FilterApply::NotApplied(code)
                if code == libc::ENOENT
                    && expected.action() == super::kqueue_change::Action::Disable =>
            {
                FilterApply::AlreadyAbsent
            }
            outcome => outcome,
        }
    }
}

/// One owned kqueue instance.
#[derive(Debug)]
pub(super) struct Kqueue {
    descriptor: OwnedFd,
}

impl Kqueue {
    pub(super) fn new() -> io::Result<Self> {
        #[cfg(target_os = "netbsd")]
        // SAFETY: kqueue1 returns a fresh descriptor adopted exactly once.
        let descriptor = unsafe {
            let raw = libc::kqueue1(libc::O_CLOEXEC);
            if raw < 0 {
                return Err(io::Error::last_os_error());
            }
            OwnedFd::from_raw_fd(raw)
        };
        #[cfg(not(target_os = "netbsd"))]
        // SAFETY: kqueue returns a fresh descriptor adopted exactly once.
        let descriptor = unsafe {
            let raw = libc::kqueue();
            if raw < 0 {
                return Err(io::Error::last_os_error());
            }
            OwnedFd::from_raw_fd(raw)
        };
        #[cfg(not(target_os = "netbsd"))]
        {
            // SAFETY: the owned descriptor and integer flag are valid for the
            // synchronous fcntl call.
            let result =
                unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) };
            if result < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(Self { descriptor })
    }

    pub(super) fn wait(
        &self,
        batch: &mut KeventBatch,
        timeout: Option<Duration>,
    ) -> io::Result<usize> {
        let timeout = timeout.map(into_timespec);
        let timeout_pointer = timeout.as_ref().map_or(ptr::null(), core::ptr::from_ref);
        #[cfg(target_os = "netbsd")]
        let max_events = batch.arena.event_capacity();
        #[cfg(not(target_os = "netbsd"))]
        let max_events = i32::try_from(batch.arena.event_capacity())
            .map_err(|_| io::Error::other("kqueue event capacity overflowed"))?;
        batch.observed = 0;
        batch.receipts = 0;
        let output = batch.arena.wait_output();
        // SAFETY: batch owns `max_events` aligned writable entries and the
        // optional timespec remains live for the synchronous call.
        let observed = unsafe {
            libc::kevent(
                self.descriptor.as_raw_fd(),
                ptr::null(),
                0,
                output.cast(),
                max_events,
                timeout_pointer,
            )
        };
        if observed < 0 {
            Err(io::Error::last_os_error())
        } else {
            let observed = usize::try_from(observed)
                .map_err(|_| io::Error::other("kqueue returned an invalid event count"))?;
            if observed > batch.arena.event_capacity() {
                return Err(io::Error::other("kqueue exceeded the event capacity"));
            }
            batch.observed = observed;
            Ok(observed)
        }
    }

    pub(super) fn submit_staged_native(
        &self,
        mut staged: StagedChanges<'_, libc::kevent>,
    ) -> NativeApply {
        let input = staged.input();
        let output = staged.output();
        let count = staged.len();
        let result = self.submit_native_pointers(input, output, count);
        if let NativeApply::Receipts(returned) = &result {
            let _ = staged.record_receipts(*returned);
        }
        result
    }

    pub(super) fn submit_native(
        &self,
        input: &[libc::kevent],
        output: &mut [libc::kevent],
    ) -> NativeApply {
        if input.is_empty() || input.len() != output.len() {
            return NativeApply::Unknown(protocol_error());
        }
        self.submit_native_pointers(input.as_ptr(), output.as_mut_ptr(), input.len())
    }

    fn submit_native_pointers(
        &self,
        input: *const libc::kevent,
        output: *mut libc::kevent,
        count: usize,
    ) -> NativeApply {
        #[cfg(target_os = "netbsd")]
        let native_count = count;
        #[cfg(not(target_os = "netbsd"))]
        let Ok(native_count) = i32::try_from(count) else {
            return NativeApply::Unknown(io::Error::from_raw_os_error(libc::EOVERFLOW));
        };
        // SAFETY: callers provide `count` initialized input entries and
        // exclusively writable output entries; no pointer escapes this call.
        let returned = unsafe {
            libc::kevent(
                self.descriptor.as_raw_fd(),
                input,
                native_count,
                output,
                native_count,
                ptr::null(),
            )
        };
        if returned < 0 {
            classify_apply_error(
                io::Error::last_os_error(),
                target_applies_interrupted_changes(),
            )
        } else if let Ok(returned) = usize::try_from(returned) {
            NativeApply::Receipts(returned)
        } else {
            NativeApply::Unknown(protocol_error())
        }
    }
}

fn initialized_event(event: &MaybeUninit<libc::kevent>) -> &libc::kevent {
    // SAFETY: callers receive slots only from arena methods that prove the
    // slot lies inside the current kernel-written prefix.
    unsafe { event.assume_init_ref() }
}

pub(super) fn empty_kevent() -> libc::kevent {
    // SAFETY: kevent contains integer fields and an inert pointer; zero is a
    // valid initialized staging value before submitted fields are assigned.
    unsafe { mem::zeroed() }
}

fn protocol_error() -> io::Error {
    io::Error::from_raw_os_error(libc::EIO)
}
