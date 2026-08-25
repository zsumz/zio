//! Opaque registration ownership and authoritative state vocabulary.

/// Opaque identity for one exact registration generation.
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
pub(crate) struct PollId(u64);

impl PollId {
    pub(crate) const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

/// Move-only capability for one registration owned by one poller.
///
/// Dropping the capability does not delete the registration. The owning poller
/// retains the descriptor until [`Poll::delete`](crate::Poll::delete) succeeds
/// or the poller itself is dropped.
#[must_use = "a registration must be passed to Poll::delete for early release"]
#[derive(Debug)]
pub struct Registration {
    owner: PollId,
    id: RegistrationId,
}

impl Registration {
    pub(crate) const fn new(owner: PollId, id: RegistrationId) -> Self {
        Self { owner, id }
    }

    pub(crate) const fn owner(&self) -> PollId {
        self.owner
    }

    /// Returns this capability's exact registration identity.
    pub const fn id(&self) -> RegistrationId {
        self.id
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
