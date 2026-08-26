//! Receipt-facing semantic metadata for benchmark scenarios.

use crate::Implementation;

use super::scenario::{ABSENCE_WINDOW_MS, BLOCKED_WAKE_SETTLE_US, Scenario, WAIT_TIMEOUT_MS};

impl Scenario {
    pub(crate) const fn semantic_scope(self) -> &'static str {
        match self {
            Self::Construct1 | Self::Construct64 | Self::Construct1024 => {
                "poller_and_event_storage_lifecycle"
            }
            Self::ConstructWaker1 | Self::ConstructWaker64 | Self::ConstructWaker1024 => {
                "poller_event_storage_and_waker_lifecycle"
            }
            Self::RegisterDelete => "readable_registration_lifecycle",
            Self::Register64 => "readable_registration_creation",
            Self::Delete64 => "readable_registration_deletion",
            Self::EmptyWait => "empty_nonblocking_observation",
            Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024 => {
                "first_readable_observation"
            }
            Self::PersistentSingle | Self::PersistentBatch64 | Self::PersistentBatch1024 => {
                "readable_observation_with_persistent_registrations"
            }
            Self::WakeRoundtrip => "pretriggered_wake_observation",
            Self::WakeBlocked => "blocked_cross_thread_wake_observation",
            Self::LevelRepeat => "native_level_repeated_observation",
            Self::OneShotRearm => "native_one_shot_rearm_then_delivery",
        }
    }

    pub(crate) const fn measurement_scope(self) -> &'static str {
        match self {
            Self::Construct1 | Self::Construct64 | Self::Construct1024 => {
                "construct_poller_and_events_then_drop"
            }
            Self::ConstructWaker1 | Self::ConstructWaker64 | Self::ConstructWaker1024 => {
                "construct_poller_events_and_waker_then_drop"
            }
            Self::RegisterDelete => "register_then_delete",
            Self::Register64 => "register_only_cleanup_untimed",
            Self::Delete64 => "delete_only_registration_untimed",
            Self::EmptyWait => "clear_events_then_nonblocking_wait",
            Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024 => {
                "register_signal_collect_drain_delete"
            }
            Self::PersistentSingle | Self::PersistentBatch64 | Self::PersistentBatch1024 => {
                "signal_collect_drain_with_registration_outside_timing"
            }
            Self::WakeRoundtrip => "trigger_then_wait_for_wake",
            Self::WakeBlocked => "wake_invocation_to_blocked_wait_return",
            Self::LevelRepeat => "wait_while_source_remains_ready",
            Self::OneShotRearm => "rearm_then_deliver",
        }
    }

    pub(crate) const fn wait_timeout_ms(self) -> Option<u64> {
        if self.is_construct()
            || matches!(
                self,
                Self::RegisterDelete | Self::Register64 | Self::Delete64
            )
        {
            None
        } else if matches!(self, Self::EmptyWait) {
            Some(0)
        } else {
            Some(WAIT_TIMEOUT_MS)
        }
    }

    pub(crate) const fn absence_window_ms(self) -> Option<u64> {
        match self {
            Self::OneShotRearm => Some(ABSENCE_WINDOW_MS),
            _ => None,
        }
    }

    pub(crate) const fn absence_window_timed(self) -> Option<bool> {
        match self {
            Self::OneShotRearm => Some(false),
            _ => None,
        }
    }

    pub(crate) const fn blocked_wake_settle_us(self) -> Option<u64> {
        match self {
            Self::WakeBlocked => Some(BLOCKED_WAKE_SETTLE_US),
            _ => None,
        }
    }

    pub(crate) const fn delivery(self) -> &'static str {
        match self {
            Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024 => "initial_observation",
            Self::PersistentSingle | Self::PersistentBatch64 | Self::PersistentBatch1024 => {
                "persistent_registration"
            }
            Self::LevelRepeat => "level",
            Self::OneShotRearm => "one_shot",
            _ => "not_applicable",
        }
    }

    pub(crate) const fn candidate_setup(self, implementation: Implementation) -> &'static str {
        match (self, implementation) {
            (
                Self::Construct1 | Self::Construct64 | Self::Construct1024,
                Implementation::Zio | Implementation::ZioBorrowed,
            ) => "eager_native_wake_source_without_public_waker",
            (Self::Construct1 | Self::Construct64 | Self::Construct1024, Implementation::Mio) => {
                "selector_and_event_storage_without_waker"
            }
            (
                Self::Construct1 | Self::Construct64 | Self::Construct1024,
                Implementation::Polling,
            ) => "poller_with_native_notify_and_event_storage",
            (Self::ConstructWaker1 | Self::ConstructWaker64 | Self::ConstructWaker1024, _) => {
                "external_usable_wake_handle_materialized"
            }
            (Self::RegisterDelete | Self::Register64 | Self::Delete64, Implementation::Zio) => {
                "retained_owned_descriptor"
            }
            (
                Self::RegisterDelete | Self::Register64 | Self::Delete64,
                Implementation::ZioBorrowed,
            ) => "caller_managed_borrowed_descriptor",
            (
                Self::RegisterDelete | Self::Register64 | Self::Delete64,
                Implementation::Mio | Implementation::Polling,
            ) => "peer_borrowed_descriptor",
            (
                Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024,
                Implementation::Zio,
            ) => "owned_descriptor_explicit_level_for_first_observation",
            (
                Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024,
                Implementation::ZioBorrowed,
            ) => "borrowed_descriptor_explicit_level_for_first_observation",
            (
                Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024,
                Implementation::Mio,
            ) => "mio_native_default",
            (
                Self::ReadySingle | Self::ReadyBatch64 | Self::ReadyBatch1024,
                Implementation::Polling,
            ) => "polling_native_default_one_shot",
            (
                Self::PersistentSingle
                | Self::PersistentBatch64
                | Self::PersistentBatch1024
                | Self::LevelRepeat,
                Implementation::Zio,
            ) => "owned_descriptor_explicit_native_level",
            (
                Self::PersistentSingle
                | Self::PersistentBatch64
                | Self::PersistentBatch1024
                | Self::LevelRepeat,
                Implementation::ZioBorrowed,
            ) => "borrowed_descriptor_explicit_native_level",
            (
                Self::PersistentSingle
                | Self::PersistentBatch64
                | Self::PersistentBatch1024
                | Self::LevelRepeat,
                Implementation::Polling,
            ) => "explicit_native_level",
            (
                Self::PersistentSingle | Self::PersistentBatch64 | Self::PersistentBatch1024,
                Implementation::Mio,
            ) => "mio_native_default_persistent_registration",
            (Self::OneShotRearm, Implementation::Zio) => {
                "owned_descriptor_explicit_native_one_shot"
            }
            (Self::OneShotRearm, Implementation::ZioBorrowed) => {
                "borrowed_descriptor_explicit_native_one_shot"
            }
            (Self::OneShotRearm, Implementation::Polling) => "explicit_native_one_shot",
            _ => "not_applicable",
        }
    }
}
