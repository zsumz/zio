//! Target-selected fixed native-event storage.

use super::event::RawEvent;

/// Fixed storage for one native kernel-event batch.
#[derive(Debug)]
pub(crate) struct RawBatch {
    #[cfg(target_os = "linux")]
    pub(super) linux: super::linux_group::RawBatch,
    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(super) kqueue: super::kqueue_group::RawBatch,
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "netbsd"
    )))]
    pub(super) unsupported: super::unsupported::RawBatch,
}

impl RawBatch {
    pub(super) fn new(events: usize, registrations: usize) -> Option<Self> {
        #[cfg(target_os = "linux")]
        {
            let _ = registrations;
            super::linux_group::Backend::raw_batch(events).map(|linux| Self { linux })
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            let _ = events;
            let capacity = registrations.checked_mul(2)?.checked_add(1)?;
            super::kqueue_group::Backend::raw_batch(capacity).map(|kqueue| Self { kqueue })
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            let _ = registrations;
            super::unsupported::Backend::raw_batch(events).map(|unsupported| Self { unsupported })
        }
    }

    pub(crate) fn event(&self, index: usize, observed: usize) -> Option<RawEvent> {
        #[cfg(target_os = "linux")]
        {
            self.linux.event(index, observed)
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            self.kqueue.event(index, observed)
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            self.unsupported.event(index, observed)
        }
    }
}
