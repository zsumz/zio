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
    pub(super) fn new(events: usize, registrations: usize) -> Result<Self, crate::Error> {
        #[cfg(target_os = "linux")]
        {
            let _ = registrations;
            super::linux_group::Backend::raw_batch(events)
                .map(|linux| Self { linux })
                .ok_or(crate::Error::Capacity {
                    kind: crate::CapacityKind::Event,
                    limit: events,
                    reason: crate::CapacityReason::BackendLimit,
                })
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            let capacity = super::batch_capacity::KqueueCapacity::new(events, registrations)?;
            super::kqueue_group::Backend::raw_batch(
                capacity.native_events(),
                capacity.native_changes(),
                capacity.recoveries(),
                capacity.arena_error(),
                capacity.recovery_error(),
            )
            .map(|kqueue| Self { kqueue })
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            let _ = registrations;
            super::unsupported::Backend::raw_batch(events)
                .map(|unsupported| Self { unsupported })
                .ok_or(crate::Error::Invariant)
        }
    }

    #[cfg(target_os = "linux")]
    #[inline]
    pub(crate) fn translate_linux<F>(
        &mut self,
        events: &mut crate::Events,
        observed: usize,
        wake_key: Option<crate::Key>,
        classify: F,
    ) -> Result<(), crate::Error>
    where
        F: FnMut(u64) -> Result<Option<(crate::Registration, crate::Key)>, crate::Error>,
    {
        self.linux.translate(events, observed, wake_key, classify)
    }

    #[cfg_attr(
        target_os = "linux",
        allow(dead_code, reason = "projection-stable non-Linux raw-event facade")
    )]
    pub(crate) fn event(&self, index: usize, observed: usize) -> Option<RawEvent> {
        #[cfg(target_os = "linux")]
        {
            let _ = (self, index, observed);
            None
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
    ) -> impl Clone + ExactSizeIterator<Item = crate::observe_recovery::DisarmOutcome> + '_ {
        self.kqueue.disarm_outcomes()
    }

    #[cfg(all(
        test,
        any(target_os = "macos", target_os = "freebsd", target_os = "netbsd")
    ))]
    pub(super) const fn native_event_capacity(&self) -> usize {
        self.kqueue.native_event_capacity()
    }
}
