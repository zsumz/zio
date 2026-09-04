//! Stable scenario catalog and per-scenario parameters.

use crate::Implementation;

pub(crate) const WAIT_TIMEOUT_MS: u64 = 1_000;
pub(crate) const ABSENCE_WINDOW_MS: u64 = 2;
pub(crate) const BLOCKED_WAKE_SETTLE_US: u64 = 200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Scenario {
    Construct1,
    Construct64,
    Construct1024,
    ConstructWaker1,
    ConstructWaker64,
    ConstructWaker1024,
    RegisterDelete,
    Register64,
    Delete64,
    EmptyWait,
    ReadySingle,
    ReadyBatch64,
    ReadyBatch1024,
    PersistentSingle,
    PersistentBatch64,
    PersistentBatch1024,
    WakeRoundtrip,
    WakeBlocked,
    LevelRepeat,
    OneShotRearm,
}

impl Scenario {
    pub(crate) const ALL: [Self; 20] = [
        Self::Construct1,
        Self::Construct64,
        Self::Construct1024,
        Self::ConstructWaker1,
        Self::ConstructWaker64,
        Self::ConstructWaker1024,
        Self::RegisterDelete,
        Self::Register64,
        Self::Delete64,
        Self::EmptyWait,
        Self::ReadySingle,
        Self::ReadyBatch64,
        Self::ReadyBatch1024,
        Self::PersistentSingle,
        Self::PersistentBatch64,
        Self::PersistentBatch1024,
        Self::WakeRoundtrip,
        Self::WakeBlocked,
        Self::LevelRepeat,
        Self::OneShotRearm,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Construct1 => "poller.construct_drop.capacity_1",
            Self::Construct64 => "poller.construct_drop.capacity_64",
            Self::Construct1024 => "poller.construct_drop.capacity_1024",
            Self::ConstructWaker1 => "poller.construct_waker_drop.capacity_1",
            Self::ConstructWaker64 => "poller.construct_waker_drop.capacity_64",
            Self::ConstructWaker1024 => "poller.construct_waker_drop.capacity_1024",
            Self::RegisterDelete => "registration.register_delete",
            Self::Register64 => "registration.register.batch_64",
            Self::Delete64 => "registration.delete.batch_64",
            Self::EmptyWait => "wait.empty.no_block",
            Self::ReadySingle => "wait.ready.readable.single.initial",
            Self::ReadyBatch64 => "wait.ready.readable.batch_64.initial",
            Self::ReadyBatch1024 => "wait.ready.readable.batch_1024.initial",
            Self::PersistentSingle => "wait.ready.readable.single.persistent",
            Self::PersistentBatch64 => "wait.ready.readable.batch_64.persistent",
            Self::PersistentBatch1024 => "wait.ready.readable.batch_1024.persistent",
            Self::WakeRoundtrip => "wake.notify.pretriggered",
            Self::WakeBlocked => "wake.notify.blocked_cross_thread",
            Self::LevelRepeat => "wait.ready.readable.level.repeat",
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
            Self::LevelRepeat | Self::OneShotRearm => {
                !matches!(implementation, Implementation::Mio)
            }
            _ => true,
        }
    }

    pub(crate) const fn is_construct(self) -> bool {
        matches!(
            self,
            Self::Construct1
                | Self::Construct64
                | Self::Construct1024
                | Self::ConstructWaker1
                | Self::ConstructWaker64
                | Self::ConstructWaker1024
        )
    }

    pub(crate) const fn constructs_waker(self) -> bool {
        matches!(
            self,
            Self::ConstructWaker1 | Self::ConstructWaker64 | Self::ConstructWaker1024
        )
    }

    pub(crate) const fn is_persistent(self) -> bool {
        matches!(
            self,
            Self::PersistentSingle | Self::PersistentBatch64 | Self::PersistentBatch1024
        )
    }

    pub(crate) const fn requires_polling_level_support(self) -> bool {
        matches!(
            self,
            Self::PersistentSingle
                | Self::PersistentBatch64
                | Self::PersistentBatch1024
                | Self::LevelRepeat
        )
    }

    pub(crate) const fn batch_size(self) -> usize {
        match self {
            Self::ReadyBatch64 | Self::PersistentBatch64 | Self::Register64 | Self::Delete64 => 64,
            Self::ReadyBatch1024 | Self::PersistentBatch1024 => 1_024,
            Self::ReadySingle | Self::PersistentSingle | Self::LevelRepeat | Self::OneShotRearm => {
                1
            }
            _ => 0,
        }
    }

    pub(crate) const fn event_capacity(self) -> usize {
        match self {
            Self::Construct64
            | Self::ConstructWaker64
            | Self::ReadyBatch64
            | Self::PersistentBatch64
            | Self::Register64
            | Self::Delete64 => 64,
            Self::Construct1024
            | Self::ConstructWaker1024
            | Self::ReadyBatch1024
            | Self::PersistentBatch1024 => 1_024,
            _ => 1,
        }
    }

    pub(crate) const fn registration_capacity(self) -> usize {
        self.event_capacity()
    }

    pub(crate) const fn default_iterations(self) -> usize {
        match self {
            Self::Register64 | Self::Delete64 => 1,
            Self::ReadyBatch64 | Self::PersistentBatch64 => 32,
            Self::ReadyBatch1024 | Self::PersistentBatch1024 => 4,
            _ => 100,
        }
    }

    pub(crate) const fn max_calibrated_iterations(self) -> usize {
        match self {
            Self::WakeBlocked => 512,
            _ => 1_000_000,
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
