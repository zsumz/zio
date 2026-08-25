//! Stable names and vocabulary for black-box wake scenarios.

/// One public-API wake behavior covered by the suite.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WakeScenario {
    /// Repeated same-key requests and explicit clones remain usable.
    SameKeyClones,
    /// A conflicting key is rejected without damaging the original waker.
    ConflictingKey,
    /// Many wakes coalesce, drain, and permit a later wake.
    PreWaitStorm,
    /// A cloned waker makes a bounded wait return with its wake event.
    CloneAcrossWait,
    /// A wake and ready resource both survive an event capacity of one.
    CapacityOneSaturation,
}

impl WakeScenario {
    /// Every V1 wake scenario in stable execution order.
    pub const ALL: [Self; 5] = [
        WAKE_SAME_KEY_CLONES,
        WAKE_CONFLICTING_KEY,
        WAKE_PRE_WAIT_STORM,
        WAKE_CLONE_ACROSS_WAIT,
        WAKE_CAPACITY_ONE_SATURATION,
    ];

    /// Returns the stable scenario name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::SameKeyClones => "wake.same_key_clones",
            Self::ConflictingKey => "wake.conflicting_key",
            Self::PreWaitStorm => "wake.pre_wait_storm",
            Self::CloneAcrossWait => "wake.clone_across_wait",
            Self::CapacityOneSaturation => "wake.capacity_one_saturation",
        }
    }
}

/// Same-key repeated requests and clones remain usable.
pub const WAKE_SAME_KEY_CLONES: WakeScenario = WakeScenario::SameKeyClones;
/// A conflicting key is rejected without damaging the original waker.
pub const WAKE_CONFLICTING_KEY: WakeScenario = WakeScenario::ConflictingKey;
/// Many pre-wait wakes coalesce and fully drain.
pub const WAKE_PRE_WAIT_STORM: WakeScenario = WakeScenario::PreWaitStorm;
/// A cloned waker remains observable across a bounded wait.
pub const WAKE_CLONE_ACROSS_WAIT: WakeScenario = WakeScenario::CloneAcrossWait;
/// Wake and resource readiness survive capacity-one saturation.
pub const WAKE_CAPACITY_ONE_SATURATION: WakeScenario = WakeScenario::CapacityOneSaturation;
