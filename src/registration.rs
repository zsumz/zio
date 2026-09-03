//! Opaque registration ownership and authoritative state vocabulary.

use std::{
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{Error, Interest, Key, Mode, token::EncodedRegistrationId};

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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
/// Ordering supports ordered containers; it does not express registration age.
#[must_use = "retain a registration handle for explicit early deletion"]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
    /// Delivery is disabled until explicitly rearmed or modified.
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

/// Ownership of the descriptor retained for a registration.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DescriptorOwnership {
    /// The poller owns and closes its retained descriptor.
    Owned,
    /// The caller owns the retained descriptor and its lifetime obligation.
    Borrowed,
}

impl RegistrationState {
    /// Returns whether backend state is known to remain registered.
    pub const fn is_registered(self) -> bool {
        matches!(self, Self::Registered { .. })
    }

    /// Returns whether backend state cannot be proven.
    pub const fn is_uncertain(self) -> bool {
        matches!(self, Self::Uncertain)
    }

    /// Returns delivery eligibility when backend state is known.
    pub const fn arm(self) -> Option<ArmState> {
        match self {
            Self::Registered { arm } => Some(arm),
            Self::Uncertain => None,
        }
    }
}

/// Poller-retained configuration and state for one registration.
///
/// An uncertain snapshot is not proof of the backend configuration.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RegistrationInfo {
    key: Key,
    interest: Interest,
    mode: Mode,
    state: RegistrationState,
    descriptor_ownership: DescriptorOwnership,
}

impl RegistrationInfo {
    pub(crate) const fn new(
        key: Key,
        interest: Interest,
        mode: Mode,
        state: RegistrationState,
        descriptor_ownership: DescriptorOwnership,
    ) -> Self {
        Self {
            key,
            interest,
            mode,
            state,
            descriptor_ownership,
        }
    }

    /// Returns the caller-selected event key.
    pub const fn key(&self) -> Key {
        self.key
    }

    /// Returns the retained readiness interest.
    pub const fn interest(&self) -> Interest {
        self.interest
    }

    /// Returns the retained delivery mode.
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// Returns the authoritative registration state.
    pub const fn state(&self) -> RegistrationState {
        self.state
    }

    /// Returns ownership of the retained descriptor.
    pub const fn descriptor_ownership(&self) -> DescriptorOwnership {
        self.descriptor_ownership
    }
}

#[cfg(test)]
#[path = "registration_test.rs"]
mod tests;
