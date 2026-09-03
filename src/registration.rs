//! Opaque registration ownership and authoritative state vocabulary.

use std::{
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{Error, token::EncodedRegistrationId};

static NEXT_POLL_ID: AtomicU64 = AtomicU64::new(1);

/// Poller-local identity for one exact registration generation.
///
/// IDs from different pollers may compare equal. Use [`Registration`] when
/// poller identity or mutation authority matters.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RegistrationId(u64);

impl RegistrationId {
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PollId(NonZeroU64);

impl PollId {
    const fn new(raw: u64) -> Option<Self> {
        match NonZeroU64::new(raw) {
            Some(raw) => Some(Self(raw)),
            None => None,
        }
    }
}

/// Lazily assigned process-unique poller authority.
#[derive(Debug)]
pub(crate) struct PollOwner(u64);

impl PollOwner {
    pub(crate) const fn unassigned() -> Self {
        Self(0)
    }

    pub(crate) const fn current(&self) -> Option<PollId> {
        PollId::new(self.0)
    }

    #[inline]
    pub(crate) fn get_or_assign(&mut self) -> Result<PollId, Error> {
        self.get_or_assign_from(&NEXT_POLL_ID)
    }

    #[inline]
    fn get_or_assign_from(&mut self, next: &AtomicU64) -> Result<PollId, Error> {
        if let Some(owner) = self.current() {
            return Ok(owner);
        }
        self.assign_from(next)
    }

    #[cold]
    #[inline(never)]
    fn assign_from(&mut self, next: &AtomicU64) -> Result<PollId, Error> {
        let raw = next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| Error::Invariant)?;
        if raw == 0 {
            return Err(Error::Invariant);
        }
        let owner = PollId::new(raw).ok_or(Error::Invariant)?;
        self.0 = raw;
        Ok(owner)
    }
}

/// Copyable handle for one exact registration generation owned by one poller.
///
/// Copying a handle does not create another registration. Once deletion is
/// proven applied, the exact generation is retired and every remaining copy is
/// stale. Dropping one or every handle does not delete the registration. The
/// poller retains an owned duplicate for [`Poll::register`](crate::Poll::register),
/// while the caller retains the descriptor for
/// [`Poll::register_borrowed`](crate::Poll::register_borrowed), until deletion
/// retires the generation or the poller itself is dropped.
#[must_use = "retain a registration handle for explicit early deletion"]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Registration {
    owner: PollId,
    id: EncodedRegistrationId,
}

impl Registration {
    pub(crate) const fn new(owner: PollId, id: EncodedRegistrationId) -> Self {
        Self { owner, id }
    }

    pub(crate) const fn from_verified(owner: PollId, id: RegistrationId) -> Self {
        Self::new(owner, EncodedRegistrationId::from_verified(id))
    }

    pub(crate) const fn owner(&self) -> PollId {
        self.owner
    }

    pub(crate) const fn encoded_id(&self) -> EncodedRegistrationId {
        self.id
    }

    /// Returns this handle's poller-local identity.
    pub const fn id(&self) -> RegistrationId {
        self.id.id()
    }

    #[cfg(test)]
    pub(crate) const fn test(id: u64) -> Self {
        Self::from_verified(PollId(NonZeroU64::MIN), RegistrationId::new(id))
    }
}

/// Whether a registered one-shot resource is eligible for delivery.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ArmState {
    /// The resource is eligible for readiness delivery.
    Armed,
    /// Delivery is disabled until an explicit modification rearms it.
    Disarmed,
}

/// Poller-authoritative state for one exact registration.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RegistrationState {
    /// The resource remains installed in the backend.
    Registered {
        /// Current delivery eligibility.
        arm: ArmState,
    },
    /// The backend state cannot be proven after a partial mutation.
    Uncertain,
}

#[cfg(test)]
#[path = "registration_test.rs"]
mod tests;
