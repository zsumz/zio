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
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    ptr,
    time::Duration,
};

use super::{
    kqueue_arena::{KqueueArena, StagedChanges},
    kqueue_codec::{NativeChange, classify_apply_error, target_applies_interrupted_changes},
    kqueue_disarm::NativeApply,
    kqueue_timeout::into_timespec,
};

#[derive(Debug)]
pub(super) struct KeventBatch {
    pub(super) arena: KqueueArena<libc::kevent>,
    pub(super) observed: usize,
    pub(super) receipts: usize,
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

    pub(super) fn submit_changes_native(&self, input: &[NativeChange]) -> NativeApply {
        if input.is_empty() {
            return NativeApply::Unknown(protocol_error());
        }
        // With no event-list slots, changelist errors are returned through
        // errno and pending readiness cannot be consumed.
        self.submit_native_counts(input.as_ptr().cast(), input.len(), ptr::null_mut(), 0)
    }

    fn submit_native_pointers(
        &self,
        input: *const libc::kevent,
        output: *mut libc::kevent,
        count: usize,
    ) -> NativeApply {
        self.submit_native_counts(input, count, output, count)
    }

    fn submit_native_counts(
        &self,
        input: *const libc::kevent,
        input_count: usize,
        output: *mut libc::kevent,
        output_count: usize,
    ) -> NativeApply {
        #[cfg(target_os = "netbsd")]
        let (native_input_count, native_output_count) = (input_count, output_count);
        #[cfg(not(target_os = "netbsd"))]
        let (Ok(native_input_count), Ok(native_output_count)) =
            (i32::try_from(input_count), i32::try_from(output_count))
        else {
            return NativeApply::Unknown(io::Error::from_raw_os_error(libc::EOVERFLOW));
        };
        // SAFETY: callers provide `input_count` initialized input entries and
        // `output_count` exclusively writable entries; null is valid for zero
        // output entries, and no pointer escapes.
        let returned = unsafe {
            libc::kevent(
                self.descriptor.as_raw_fd(),
                input,
                native_input_count,
                output,
                native_output_count,
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

pub(super) fn initialized_event(event: &MaybeUninit<libc::kevent>) -> &libc::kevent {
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
