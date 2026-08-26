//! Lifetime guard for `polling`'s borrowed-source registration capability.

use std::{os::unix::net::UnixStream, sync::Arc};

use polling::{Event, PollMode, Poller};

pub(crate) struct PollingRegistration<'poller, 'source> {
    poller: PollerOwner<'poller>,
    source: &'source UnixStream,
    active: bool,
}

impl<'poller, 'source> PollingRegistration<'poller, 'source> {
    pub(crate) fn shared(
        poller: Arc<Poller>,
        source: &'source UnixStream,
        event: Event,
        mode: PollMode,
    ) -> Result<PollingRegistration<'static, 'source>, std::io::Error> {
        let mut registration = PollingRegistration {
            poller: PollerOwner::Shared(poller),
            source,
            active: false,
        };
        add(registration.poller(), registration.source, event, mode)?;
        registration.active = true;
        Ok(registration)
    }

    pub(crate) fn borrowed(
        poller: &'poller Poller,
        source: &'source UnixStream,
        event: Event,
        mode: PollMode,
    ) -> Result<Self, std::io::Error> {
        let mut registration = Self {
            poller: PollerOwner::Borrowed(poller),
            source,
            active: false,
        };
        add(registration.poller(), registration.source, event, mode)?;
        registration.active = true;
        Ok(registration)
    }

    pub(crate) fn poller(&self) -> &Poller {
        match &self.poller {
            PollerOwner::Borrowed(poller) => poller,
            PollerOwner::Shared(poller) => poller,
        }
    }

    #[cfg(test)]
    pub(crate) fn source(&self) -> &UnixStream {
        self.source
    }

    pub(crate) fn modify(&self, event: Event, mode: PollMode) -> Result<(), std::io::Error> {
        self.poller().modify_with_mode(self.source, event, mode)
    }

    pub(crate) fn delete(mut self) -> Result<(), std::io::Error> {
        let exclusive_owner = self.poller.has_exclusive_shared_owner();
        let result = self.poller().delete(self.source);
        if result.is_ok() || exclusive_owner {
            // An exclusively owned poller closes before the source borrow can
            // end, so even an unsuccessful delete cannot retain the source.
            self.active = false;
        }
        result
    }
}

impl Drop for PollingRegistration<'_, '_> {
    fn drop(&mut self) {
        if self.active {
            if self.poller().delete(self.source).is_err() {
                // Returning would let the borrowed source close while the
                // backend may still retain its descriptor. Termination is the
                // only contract-preserving response available from Drop.
                std::process::abort();
            }
            self.active = false;
        }
    }
}

enum PollerOwner<'poller> {
    Borrowed(&'poller Poller),
    Shared(Arc<Poller>),
}

impl PollerOwner<'_> {
    fn has_exclusive_shared_owner(&self) -> bool {
        match self {
            Self::Borrowed(_) => false,
            Self::Shared(poller) => Arc::strong_count(poller) == 1,
        }
    }
}

#[allow(
    unsafe_code,
    reason = "the returned active guard keeps source and poller alive until delete or poller drop"
)]
fn add(
    poller: &Poller,
    source: &UnixStream,
    event: Event,
    mode: PollMode,
) -> Result<(), std::io::Error> {
    // SAFETY: storage for the inactive guard is allocated before this call.
    // Success marks it active; its lifetimes keep `source` and `poller` alive.
    // It deletes before releasing either borrow. An exclusive owned poller is
    // instead closed while `source` is still borrowed if delete fails.
    unsafe { poller.add_with_mode(source, event, mode) }
}
