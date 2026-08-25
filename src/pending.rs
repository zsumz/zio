//! Target-selected storage for coalescing split native observations.

use core::num::NonZeroUsize;

/// Fixed aggregation storage whose reset cost follows observed events.
#[derive(Debug)]
pub(crate) struct PendingBatch {
    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    inner: crate::pending_kqueue::KqueuePending,
}

impl PendingBatch {
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd")),
        allow(clippy::unnecessary_wraps, reason = "kqueue allocation is fallible")
    )]
    pub(crate) fn new(
        capacity: NonZeroUsize,
        registrations: NonZeroUsize,
    ) -> Result<Self, crate::Error> {
        #[cfg(not(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd")))]
        let _ = (capacity, registrations);
        Ok(Self {
            #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
            inner: crate::pending_kqueue::KqueuePending::new(capacity, registrations)?,
        })
    }

    pub(crate) fn clear(&mut self) {
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        self.inner.clear();
        #[cfg(not(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd")))]
        let _ = self;
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(crate) fn add(
        &mut self,
        registration: crate::RegistrationId,
        key: crate::Key,
        readiness: crate::Readiness,
    ) -> Result<(), crate::Error> {
        self.inner.add(registration, key, readiness)
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(crate) fn as_slice(&self) -> &[crate::pending_kqueue::PendingResource] {
        self.inner.as_slice()
    }
}
