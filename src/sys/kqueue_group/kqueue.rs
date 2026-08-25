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
    kqueue_change::{Action, Change, ChangeList, Filter, RawKevent, Receipt, Receipts},
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
            has_flag(u64::from(event.flags), u64::from(libc::EV_ERROR)) || event.fflags != 0,
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
        #[cfg(target_os = "netbsd")]
        let count = changes.len();
        #[cfg(not(target_os = "netbsd"))]
        let count = i32::try_from(changes.len())
            .map_err(|_| io::Error::other("kqueue change count overflowed"))?;
        let mut input = [empty_kevent(), empty_kevent()];
        let mut output = [empty_kevent(), empty_kevent()];
        for (destination, change) in input.iter_mut().zip(changes) {
            *destination = encode_change(*change)?;
        }
        // SAFETY: input contains `count` initialized values and output owns the
        // same count of writable values; neither pointer escapes.
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
            return Err(io::Error::last_os_error());
        }
        if usize::try_from(returned).ok() != Some(changes.len()) {
            return Err(io::Error::other(
                "kqueue returned an incomplete receipt set",
            ));
        }
        let mut receipts = Receipts::new(changes.len());
        for (index, (event, change)) in output.into_iter().zip(changes).enumerate() {
            let error = if has_flag(u64::from(event.flags), u64::from(libc::EV_ERROR)) {
                i32::try_from(event.data).ok().filter(|code| *code != 0)
            } else {
                Some(libc::EIO)
            };
            receipts.set(index, Receipt::new(change.action(), error))?;
        }
        Ok(receipts)
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
}

fn empty_kevent() -> libc::kevent {
    // SAFETY: kevent contains integer fields and an inert pointer; zero is a
    // valid initialized staging value before submitted fields are assigned.
    unsafe { mem::zeroed() }
}

fn encode_change(change: Change) -> io::Result<libc::kevent> {
    let mut event = empty_kevent();
    event.ident = libc::uintptr_t::try_from(change.ident())
        .map_err(|_| io::Error::other("negative kqueue identifier"))?;
    event.filter = match change.filter() {
        Filter::Read => libc::EVFILT_READ,
        Filter::Write => libc::EVFILT_WRITE,
        Filter::User => libc::EVFILT_USER,
        Filter::Unknown => 0,
    };
    let receipt = libc::EV_RECEIPT;
    event.flags = match change.action() {
        Action::AddEnabled => libc::EV_ADD | libc::EV_ENABLE | receipt,
        Action::AddDisabled => libc::EV_ADD | libc::EV_DISABLE | receipt,
        Action::AddUser => libc::EV_ADD | libc::EV_CLEAR | receipt,
        Action::Delete => libc::EV_DELETE | receipt,
        Action::Disable => libc::EV_DISABLE | receipt,
        Action::Trigger => libc::EV_ADD | receipt,
    };
    event.fflags = u32::from(change.action() == Action::Trigger) * libc::NOTE_TRIGGER;
    event.udata = usize::try_from(change.token())
        .map_err(|_| io::Error::other("kqueue token exceeds pointer width"))?
        as *mut libc::c_void;
    Ok(event)
}

fn decode_filter(filter: i64) -> Filter {
    if filter == i64::from(libc::EVFILT_READ) {
        Filter::Read
    } else if filter == i64::from(libc::EVFILT_WRITE) {
        Filter::Write
    } else if filter == i64::from(libc::EVFILT_USER) {
        Filter::User
    } else {
        Filter::Unknown
    }
}

fn has_flag(flags: u64, expected: u64) -> bool {
    flags & expected != 0
}
