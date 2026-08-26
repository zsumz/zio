//! Owned kqueue descriptor and reviewed syscall leaf.

#![allow(
    unsafe_code,
    reason = "reviewed kqueue FFI is confined to this syscall leaf"
)]

#[cfg(not(target_pointer_width = "64"))]
const _: [(); 8] = [(); core::mem::size_of::<usize>()];

use std::{
    io, mem,
    os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    ptr,
    time::Duration,
};

use super::{
    kqueue_change::{Change, ChangeList, RawKevent, Receipt, Receipts},
    kqueue_codec::{
        KeventChangeBatch, classify_apply_error, decode_filter, decode_receipt, encode_change,
        has_flag, target_applies_interrupted_changes,
    },
    kqueue_disarm::{FilterApply, NativeApply},
    kqueue_timeout::into_timespec,
};

/// Fixed initialized output storage written only by `kevent`.
pub(super) struct KeventBatch {
    events: Box<[libc::kevent]>,
    max_events: usize,
}

impl KeventBatch {
    pub(super) fn new(capacity: usize) -> Option<Self> {
        if capacity == 0 {
            return None;
        }
        #[cfg(not(target_os = "netbsd"))]
        i32::try_from(capacity).ok()?;
        let mut events = Vec::new();
        events.try_reserve_exact(capacity).ok()?;
        events.resize_with(capacity, empty_kevent);
        Some(Self {
            events: events.into_boxed_slice(),
            max_events: capacity,
        })
    }

    pub(super) fn event(&self, index: usize, observed: usize) -> Option<RawKevent> {
        if index >= observed {
            return None;
        }
        let event = self.events.get(index)?;
        Some(RawKevent::new(
            RawFd::try_from(event.ident).ok()?,
            decode_filter(i64::from(event.filter)),
            event.udata as usize as u64,
            has_flag(u64::from(event.flags), u64::from(libc::EV_EOF)),
            has_flag(u64::from(event.flags), u64::from(libc::EV_ERROR)),
            event.fflags,
        ))
    }
}

impl std::fmt::Debug for KeventBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeventBatch")
            .field("capacity", &self.events.len())
            .finish_non_exhaustive()
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
        // SAFETY: kqueue1 receives one documented descriptor flag.
        let descriptor = unsafe { libc::kqueue1(libc::O_CLOEXEC) };
        #[cfg(not(target_os = "netbsd"))]
        // SAFETY: kqueue takes no arguments and returns a fresh descriptor.
        let descriptor = unsafe { libc::kqueue() };
        if descriptor < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: ownership of the fresh descriptor is transferred once.
        let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor) };
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

    pub(super) fn apply(&self, changes: &ChangeList) -> io::Result<Receipts> {
        let changes = changes.as_slice();
        let mut input = [empty_kevent(), empty_kevent()];
        let mut output = [empty_kevent(), empty_kevent()];
        for (destination, change) in input.iter_mut().zip(changes) {
            *destination = encode_change(*change)?;
        }
        let mut receipts = Receipts::new(changes.len());
        match self.submit_native(&input[..changes.len()], &mut output[..changes.len()]) {
            NativeApply::AppliedWithoutReceipts => {
                for (index, change) in changes.iter().copied().enumerate() {
                    receipts.set(index, Receipt::new(change.action(), None))?;
                }
            }
            NativeApply::Receipts(returned) if returned == changes.len() => {
                for (index, change) in changes.iter().copied().enumerate() {
                    let error = match decode_receipt(output.get(index), change) {
                        FilterApply::Applied => None,
                        FilterApply::NotApplied(code) => Some(code),
                        FilterApply::AlreadyAbsent | FilterApply::Unknown(_) => {
                            return Err(protocol_error());
                        }
                    };
                    receipts.set(index, Receipt::new(change.action(), error))?;
                }
            }
            NativeApply::Receipts(_) => return Err(protocol_error()),
            NativeApply::Unknown(source) => return Err(source),
        }
        Ok(receipts)
    }

    pub(super) fn apply_batch(
        &self,
        changes: &[Change],
        batch: &mut KeventChangeBatch,
    ) -> NativeApply {
        if changes.len() > batch.input.len() {
            return NativeApply::Unknown(protocol_error());
        }
        for (destination, change) in batch.input.iter_mut().zip(changes) {
            let Ok(encoded) = encode_change(*change) else {
                return NativeApply::Unknown(io::Error::from_raw_os_error(libc::EINVAL));
            };
            *destination = encoded;
        }
        self.submit_native(
            &batch.input[..changes.len()],
            &mut batch.output[..changes.len()],
        )
    }

    pub(super) fn wait(
        &self,
        batch: &mut KeventBatch,
        timeout: Option<Duration>,
    ) -> io::Result<usize> {
        let timeout = timeout.map(into_timespec);
        let timeout_pointer = timeout.as_ref().map_or(ptr::null(), core::ptr::from_ref);
        #[cfg(target_os = "netbsd")]
        let max_events = batch.max_events;
        #[cfg(not(target_os = "netbsd"))]
        let max_events = i32::try_from(batch.max_events)
            .map_err(|_| io::Error::other("kqueue event capacity overflowed"))?;
        // SAFETY: batch owns `max_events` initialized writable entries and the
        // optional timespec remains live for the synchronous call.
        let observed = unsafe {
            libc::kevent(
                self.descriptor.as_raw_fd(),
                ptr::null(),
                0,
                batch.events.as_mut_ptr(),
                max_events,
                timeout_pointer,
            )
        };
        if observed < 0 {
            Err(io::Error::last_os_error())
        } else {
            usize::try_from(observed)
                .map_err(|_| io::Error::other("kqueue returned an invalid event count"))
        }
    }

    fn submit_native(&self, input: &[libc::kevent], output: &mut [libc::kevent]) -> NativeApply {
        if input.is_empty() || input.len() != output.len() {
            return NativeApply::Unknown(protocol_error());
        }
        #[cfg(target_os = "netbsd")]
        let count = input.len();
        #[cfg(not(target_os = "netbsd"))]
        let Ok(count) = i32::try_from(input.len()) else {
            return NativeApply::Unknown(io::Error::from_raw_os_error(libc::EOVERFLOW));
        };
        // SAFETY: both slices contain `count` initialized entries; output is
        // exclusively writable for the synchronous call and no pointer escapes.
        let returned = unsafe {
            libc::kevent(
                self.descriptor.as_raw_fd(),
                input.as_ptr(),
                count,
                output.as_mut_ptr(),
                count,
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

pub(super) fn empty_kevent() -> libc::kevent {
    // SAFETY: kevent contains integer fields and an inert pointer; zero is a
    // valid initialized staging value before submitted fields are assigned.
    unsafe { mem::zeroed() }
}

fn protocol_error() -> io::Error {
    io::Error::from_raw_os_error(libc::EIO)
}
