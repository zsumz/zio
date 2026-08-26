//! Stable scenario catalog and per-scenario parameters.

use crate::Implementation;

pub(crate) const WAIT_TIMEOUT_MS: u64 = 1_000;
pub(crate) const ABSENCE_WINDOW_MS: u64 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Scenario {
    ConstructDrop,
    RegisterDelete,
    EmptyWait,
    ReadySingle,
    ReadyBatch64,
    ReadyBatch1024,
    WakeRoundtrip,
    LevelRepeat,
    OneShotDisarm,
    OneShotRearm,
}

impl Scenario {
    pub(crate) const ALL: [Self; 10] = [
        Self::ConstructDrop,
        Self::RegisterDelete,
        Self::EmptyWait,
        Self::ReadySingle,
        Self::ReadyBatch64,
        Self::ReadyBatch1024,
        Self::WakeRoundtrip,
        Self::LevelRepeat,
        Self::OneShotDisarm,
        Self::OneShotRearm,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::ConstructDrop => "poller.construct_drop",
            Self::RegisterDelete => "registration.register_delete",
            Self::EmptyWait => "wait.empty.no_block",
            Self::ReadySingle => "wait.ready.readable.single.initial",
            Self::ReadyBatch64 => "wait.ready.readable.batch_64.initial",
            Self::ReadyBatch1024 => "wait.ready.readable.batch_1024.initial",
            Self::WakeRoundtrip => "wake.notify.roundtrip",
            Self::LevelRepeat => "wait.ready.readable.level.repeat",
            Self::OneShotDisarm => "wait.ready.readable.one_shot.disarm",
            Self::OneShotRearm => "wait.ready.readable.one_shot.rearm",
        }
    }

    pub(crate) const fn parse(value: &str) -> Option<Self> {
        let mut index = 0;
        while index < Self::ALL.len() {
            let scenario = Self::ALL[index];
            if equal(scenario.name(), value) {
                return Some(scenario);
            }
            index += 1;
        }
        None
    }

    pub(crate) const fn supports(self, implementation: Implementation) -> bool {
        match self {
            Self::LevelRepeat | Self::OneShotDisarm | Self::OneShotRearm => {
                !matches!(implementation, Implementation::Mio)
            }
            _ => true,
        }
    }

    pub(crate) const fn batch_size(self) -> usize {
        match self {
            Self::ReadyBatch64 => 64,
            Self::ReadyBatch1024 => 1_024,
            Self::ReadySingle | Self::LevelRepeat | Self::OneShotDisarm | Self::OneShotRearm => 1,
            _ => 0,
        }
    }

    pub(crate) const fn event_capacity(self) -> usize {
        match self {
            Self::ConstructDrop | Self::ReadyBatch1024 => 1_024,
            Self::ReadyBatch64 => 64,
            _ => 1,
        }
    }

    pub(crate) const fn registration_capacity(self) -> usize {
        match self {
            Self::ConstructDrop | Self::ReadyBatch1024 => 1_024,
            Self::ReadyBatch64 => 64,
            _ => 1,
        }
    }

    pub(crate) const fn default_iterations(self) -> usize {
        match self {
            Self::ReadyBatch64 => 32,
            Self::ReadyBatch1024 => 4,
            Self::OneShotDisarm => 10,
            _ => 100,
        }
    }

    pub(crate) const fn semantic_scope(self) -> &'static str {
        match self {
            Self::ConstructDrop => "poller_and_event_storage_lifecycle",
            Self::RegisterDelete => "readable_registration_lifecycle",
            Self::EmptyWait => "empty_nonblocking_observation",
            Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024 => {
                "first_readable_observation"
            }
            Self::WakeRoundtrip => "pretriggered_wake_observation",
            Self::LevelRepeat => "native_level_repeated_observation",
            Self::OneShotDisarm => "native_one_shot_delivery_then_absence",
            Self::OneShotRearm => "native_one_shot_rearm_then_delivery",
        }
    }

    pub(crate) const fn measurement_scope(self) -> &'static str {
        match self {
            Self::ConstructDrop => "construct_poller_and_events_then_drop",
            Self::RegisterDelete => "register_then_delete",
            Self::EmptyWait => "clear_events_then_nonblocking_wait",
            Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024 => {
                "register_signal_collect_drain_delete"
            }
            Self::WakeRoundtrip => "trigger_then_wait_for_wake",
            Self::LevelRepeat => "wait_while_source_remains_ready",
            Self::OneShotDisarm => "rearm_deliver_then_positive_absence_probe",
            Self::OneShotRearm => "rearm_then_deliver",
        }
    }

    pub(crate) const fn wait_timeout_ms(self) -> Option<u64> {
        match self {
            Self::ConstructDrop | Self::RegisterDelete => None,
            Self::EmptyWait => Some(0),
            Self::ReadySingle
            | Self::ReadyBatch64
            | Self::ReadyBatch1024
            | Self::WakeRoundtrip
            | Self::LevelRepeat
            | Self::OneShotDisarm
            | Self::OneShotRearm => Some(WAIT_TIMEOUT_MS),
        }
    }

    pub(crate) const fn absence_window_ms(self) -> Option<u64> {
        match self {
            Self::OneShotDisarm | Self::OneShotRearm => Some(ABSENCE_WINDOW_MS),
            _ => None,
        }
    }

    pub(crate) const fn absence_window_timed(self) -> Option<bool> {
        match self {
            Self::OneShotDisarm => Some(true),
            Self::OneShotRearm => Some(false),
            _ => None,
        }
    }

    pub(crate) const fn delivery(self) -> &'static str {
        match self {
            Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024 => "initial_observation",
            Self::LevelRepeat => "level",
            Self::OneShotDisarm | Self::OneShotRearm => "one_shot",
            _ => "not_applicable",
        }
    }

    pub(crate) const fn candidate_setup(self, implementation: Implementation) -> &'static str {
        match (self, implementation) {
            (
                Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024,
                Implementation::Zio,
            ) => "explicit_level_for_first_observation",
            (
                Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024,
                Implementation::Mio,
            ) => "mio_native_default",
            (
                Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024,
                Implementation::Polling,
            ) => "polling_native_default_one_shot",
            (Self::LevelRepeat, _) => "explicit_native_level",
            (Self::OneShotDisarm | Self::OneShotRearm, _) => "explicit_native_one_shot",
            _ => "not_applicable",
        }
    }
}

const fn equal(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}
