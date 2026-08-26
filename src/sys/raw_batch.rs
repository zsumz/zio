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
            let capacity = registrations.checked_mul(2)?.checked_add(1)?;
            let disarms = events.min(registrations);
            super::kqueue_group::Backend::raw_batch(capacity, disarms).map(|kqueue| Self { kqueue })
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

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(crate) fn clear_disarms(&mut self) {
        self.kqueue.clear_disarms();
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(crate) fn push_disarm(
        &mut self,
        registration: crate::RegistrationId,
        descriptor: std::os::fd::RawFd,
        interest: crate::Interest,
    ) -> Option<()> {
        self.kqueue.push_disarm(registration, descriptor, interest)
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(crate) fn disarm_outcomes(
        &self,
    ) -> impl Clone + ExactSizeIterator<Item = crate::RecoveryOutcome> + '_ {
        self.kqueue.disarm_outcomes()
    }
}
