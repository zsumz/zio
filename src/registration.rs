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

/// Copyable handle for one exact registration generation owned by one poller.
///
/// Copying a handle does not create another registration. Once deletion is
/// proven applied, the exact generation is retired and every remaining copy is
/// stale. Dropping one or every handle does not delete the registration; the
/// owning poller retains the descriptor until deletion retires the generation
/// or the poller itself is dropped.
#[must_use = "retain a registration handle for explicit early deletion"]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

    /// Returns this handle's exact registration identity.
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
