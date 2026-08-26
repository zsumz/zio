//! Vendor-neutral qualification vocabulary.

/// A readiness implementation evaluated by the harness.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Implementation {
    /// Zio's public poller.
    Zio,
    /// Zio's explicit borrowed-registration tier.
    ZioBorrowed,
    /// Mio's public poller.
    Mio,
    /// The `polling` crate's public poller.
    Polling,
}

impl Implementation {
    /// Every candidate in stable report order.
    pub const ALL: [Self; 4] = [Self::Zio, Self::ZioBorrowed, Self::Mio, Self::Polling];

    /// Returns the stable receipt name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Zio => "zio",
            Self::ZioBorrowed => "zio-borrowed",
            Self::Mio => "mio",
            Self::Polling => "polling",
        }
    }
}

/// Delivery semantics covered by a scenario.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeliveryProfile {
    /// Only the first observation is in scope; each API uses its stated setup.
    InitialObservation,
    /// Native level-triggered repeated delivery is required.
    Level,
    /// Native one-shot disarm and explicit rearm are required.
    OneShot,
}

/// Delivery semantics actually configured through a candidate API.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConfiguredDelivery {
    /// Explicit level-triggered delivery.
    Level,
    /// Explicit one-shot delivery.
    OneShot,
    /// Candidate-native delivery without a selectable mode.
    NativeDefault,
}

impl ConfiguredDelivery {
    /// Returns the stable receipt label.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Level => "level",
            Self::OneShot => "one_shot",
            Self::NativeDefault => "native_default",
        }
    }
}

/// Whether a candidate exposes a delivery profile on this host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileSupport {
    /// The library natively exposes the requested profile.
    Native,
    /// The library does not expose this semantic axis.
    NotExposed {
        /// Stable explanation suitable for a receipt.
        reason: &'static str,
    },
    /// The library exposes the profile, but the host backend does not.
    HostUnavailable {
        /// Stable explanation suitable for a receipt.
        reason: &'static str,
    },
}

impl ProfileSupport {
    /// Returns a stable capability label.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::NotExposed { .. } => "not_exposed",
            Self::HostUnavailable { .. } => "host_unavailable",
        }
    }

    /// Returns the stable reason when the profile cannot run.
    pub const fn reason(self) -> Option<&'static str> {
        match self {
            Self::Native => None,
            Self::NotExposed { reason } | Self::HostUnavailable { reason } => Some(reason),
        }
    }
}

/// Readiness direction exercised by a fixture.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Interest {
    /// Readable progress.
    Readable,
    /// Writable progress.
    Writable,
}

/// One stable native qualification scenario.
#[allow(
    clippy::enum_variant_names,
    reason = "the Unix prefix makes the transport explicit in public receipts"
)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Scenario {
    /// Initial readable observation under each API's stated setup.
    UnixReadableInitial,
    /// Initial writable observation under each API's stated setup.
    UnixWritableInitial,
    /// Repeated native level-readable delivery.
    UnixReadableLevel,
    /// Repeated native level-writable delivery.
    UnixWritableLevel,
    /// Native one-shot readable disarm and rearm.
    UnixReadableOneShot,
    /// Native one-shot writable disarm and rearm.
    UnixWritableOneShot,
}

impl Scenario {
    /// Every scenario in stable receipt order.
    pub const ALL: [Self; 6] = [
        Self::UnixReadableInitial,
        Self::UnixWritableInitial,
        Self::UnixReadableLevel,
        Self::UnixWritableLevel,
        Self::UnixReadableOneShot,
        Self::UnixWritableOneShot,
    ];

    /// Returns the stable scenario name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::UnixReadableInitial => "unix.readable.initial_observation",
            Self::UnixWritableInitial => "unix.writable.initial_observation",
            Self::UnixReadableLevel => "unix.readable.level",
            Self::UnixWritableLevel => "unix.writable.level",
            Self::UnixReadableOneShot => "unix.readable.one_shot",
            Self::UnixWritableOneShot => "unix.writable.one_shot",
        }
    }

    /// Returns the fixture direction.
    pub const fn interest(self) -> Interest {
        match self {
            Self::UnixReadableInitial | Self::UnixReadableLevel | Self::UnixReadableOneShot => {
                Interest::Readable
            }
            Self::UnixWritableInitial | Self::UnixWritableLevel | Self::UnixWritableOneShot => {
                Interest::Writable
            }
        }
    }

    /// Returns the delivery profile under qualification.
    pub const fn profile(self) -> DeliveryProfile {
        match self {
            Self::UnixReadableInitial | Self::UnixWritableInitial => {
                DeliveryProfile::InitialObservation
            }
            Self::UnixReadableLevel | Self::UnixWritableLevel => DeliveryProfile::Level,
            Self::UnixReadableOneShot | Self::UnixWritableOneShot => DeliveryProfile::OneShot,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RegistrationSpec {
    pub(crate) key: usize,
    pub(crate) interest: Interest,
    pub(crate) profile: DeliveryProfile,
}
