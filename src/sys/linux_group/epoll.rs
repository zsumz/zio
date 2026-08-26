//! Caller-event arena reuse and reviewed epoll syscall leaf.

#![allow(
    unsafe_code,
    reason = "reviewed epoll FFI and arena transitions are confined to this leaf"
)]

use std::{
    io,
    mem::{align_of, needs_drop, size_of},
    os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd},
};

use crate::{Error, Event, Events, Key};

/// Fixed-capacity state for raw observations staged in caller event storage.
pub(super) struct EpollBatch {
    max_events: libc::c_int,
    capacity: usize,
    native_offset: usize,
    observed: usize,
    storage_address: usize,
}

impl EpollBatch {
    pub(super) fn new(capacity: usize) -> Option<Self> {
        let max_events = libc::c_int::try_from(capacity)
            .ok()
            .filter(|count| *count > 0)?;
        if size_of::<Event>() < size_of::<libc::epoll_event>()
            || align_of::<Event>() < align_of::<libc::epoll_event>()
            || needs_drop::<Event>()
        {
            return None;
        }
        let gap = size_of::<Event>().checked_sub(size_of::<libc::epoll_event>())?;
        let native_offset = capacity.checked_mul(gap)?;
        if native_offset % align_of::<libc::epoll_event>() != 0 {
            return None;
        }
        Some(Self {
            max_events,
            capacity,
            native_offset,
            observed: 0,
            storage_address: 0,
        })
    }

    #[inline]
    pub(super) fn translate<F>(
        &mut self,
        events: &mut Events,
        observed: usize,
        wake_key: Option<Key>,
        mut classify: F,
    ) -> Result<(), Error>
    where
        F: FnMut(u64) -> Result<Option<Key>, Error>,
    {
        let capacity = self.capacity();
        if self.storage_address == 0
            || !events.is_empty()
            || events.capacity() < capacity
            || observed != self.observed
            || observed > capacity
        {
            self.invalidate();
            return Err(Error::Invariant);
        }
        let storage = events.linux_storage();
        if storage.as_ptr().addr() != self.storage_address {
            self.invalidate();
            return Err(Error::Invariant);
        }
        let native = self.native_storage(storage);
        self.invalidate();
        let portable = storage.as_mut_ptr();
        let mut resources = 0;
        let mut woke = false;
        for source in 0..observed {
            // SAFETY: the stamped successful wait or test staging initialized
            // this native-stride prefix, and `source` is inside that prefix.
            let raw = unsafe { native.add(source).read() };
            if raw.u64 == 0 {
                woke = true;
                continue;
            }
            let key = classify(raw.u64)?;
            if let Some(key) = key {
                let event = Event::Resource {
                    key,
                    readiness: super::backend::from_epoll_flags(raw.events),
                };
                // SAFETY: raw observations occupy the configured arena's
                // native tail. Since `resources <= source`, this write ends
                // before the next raw record, or overlaps only the current
                // record after it was copied.
                unsafe { portable.add(resources).write(event) };
                resources += 1;
            }
        }
        let wake = if woke { wake_key } else { None };
        let output = resources + usize::from(wake.is_some());
        if let Some(key) = wake {
            // SAFETY: classification is complete and `output <= capacity`, so
            // this is the initialized prefix's final in-bounds Event slot.
            unsafe { portable.add(resources).write(Event::Wake { key }) };
        }
        // SAFETY: the forward loop initialized every resource slot and the
        // optional wake initialized the only remaining committed slot. Event
        // has no drop glue, so an earlier classification error leaves no leak.
        unsafe { storage.set_len(output) };
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn stage(&mut self, events: &mut Events, raw: &[(u32, u64)]) -> Option<usize> {
        self.invalidate();
        let capacity = self.capacity();
        if !events.is_empty() || raw.len() > capacity || events.capacity() < capacity {
            return None;
        }
        let storage = events.linux_storage();
        let native = self.native_storage(storage);
        self.storage_address = storage.as_ptr().addr();
        self.observed = raw.len();
        for (index, (flags, token)) in raw.iter().copied().enumerate() {
            let event = libc::epoll_event {
                events: flags,
                u64: token,
            };
            // SAFETY: the layout and capacity checks match the wait boundary.
            unsafe { native.add(index).write(event) };
        }
        Some(raw.len())
    }

    fn invalidate(&mut self) {
        self.observed = 0;
        self.storage_address = 0;
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn native_storage(&self, storage: &mut Vec<Event>) -> *mut libc::epoll_event {
        let native = storage
            .as_mut_ptr()
            .cast::<u8>()
            .wrapping_add(self.native_offset)
            .cast::<libc::epoll_event>();
        debug_assert_eq!(native.align_offset(align_of::<libc::epoll_event>()), 0);
        native
    }
}

impl std::fmt::Debug for EpollBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EpollBatch")
            .field("capacity", &self.capacity())
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

    pub(super) fn wait(
        &self,
        batch: &mut EpollBatch,
        events: &mut Events,
        timeout: libc::c_int,
    ) -> io::Result<usize> {
        batch.invalidate();
        if !events.is_empty() || events.capacity() < batch.capacity() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "epoll event storage is not empty or is undersized",
            ));
        }
        let storage = events.linux_storage();
        let native = batch.native_storage(storage);
        // SAFETY: `new` checked the Event/native layout, the empty Vec owns at
        // least `max_events` tail-aligned writable slots, and epoll retains no
        // pointer.
        let observed = unsafe {
            libc::epoll_wait(
                self.descriptor.as_raw_fd(),
                native,
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
            batch.storage_address = storage.as_ptr().addr();
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
