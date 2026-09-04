//! Portable value and capability trait contracts.

use zio::{
    ArmState, CapacityKind, CapacityReason, CommitStatus, DescriptorOwnership, Event, Events, Key,
    Mode, Operation, Poll, PollBuilder, Readiness, RecoveryFailure, RecoveryOutcome, Registration,
    RegistrationId, RegistrationInfo, RegistrationState, Wait, WaitReport, Waker,
};

use super::support::*;

#[test]
fn public_values_keep_common_traits() {
    assert_copy_value::<Key>();
    assert_copy_value::<zio::Interest>();
    assert_copy_value::<Readiness>();
    assert_copy_value::<Event>();
    assert_copy_value::<Mode>();
    assert_copy_value::<Wait>();
    assert_copy_value::<Registration>();
    assert_copy_value::<RegistrationId>();
    assert_copy_value::<RegistrationInfo>();
    assert_copy_value::<RecoveryOutcome>();
    assert_copy_value::<ArmState>();
    assert_copy_value::<RegistrationState>();
    assert_copy_value::<DescriptorOwnership>();
    assert_copy_value::<Operation>();
    assert_copy_value::<CommitStatus>();
    assert_copy_value::<CapacityKind>();
    assert_copy_value::<CapacityReason>();
    assert_copy_value::<PollBuilder>();
    assert_debug_thread_value::<Events>();
    assert_debug_thread_value::<WaitReport>();
}

#[test]
fn public_defaults_are_named_values() {
    assert_eq!(Key::default(), Key::ZERO);
    assert_eq!(zio::Interest::default(), zio::Interest::EMPTY);
    assert_eq!(Readiness::default(), Readiness::EMPTY);
    assert_eq!(PollBuilder::default(), PollBuilder::new());
}

#[test]
fn fixed_batch_cardinality_queries_are_const() {
    const fn event_cardinality(events: &Events) -> (usize, bool, usize, bool) {
        (
            events.len(),
            events.is_empty(),
            events.remaining_capacity(),
            events.is_full(),
        )
    }

    const fn recovery_cardinality(recovery: &RecoveryFailure) -> (usize, bool) {
        (recovery.len(), recovery.is_empty())
    }

    let _ = event_cardinality as fn(&Events) -> (usize, bool, usize, bool);
    let _ = recovery_cardinality as fn(&RecoveryFailure) -> (usize, bool);
}

#[test]
fn fixed_batch_slice_queries_are_const() {
    const fn event_slice(events: &Events) -> &[Event] {
        events.as_slice()
    }

    const fn recovery_slice(recovery: &RecoveryFailure) -> &[RecoveryOutcome] {
        recovery.outcomes()
    }

    let _ = event_slice as fn(&Events) -> &[Event];
    let _ = recovery_slice as fn(&RecoveryFailure) -> &[RecoveryOutcome];
}

#[test]
fn registration_handles_support_ordered_collections() {
    let _ = Registration::id as fn(&Registration) -> RegistrationId;
    assert_ordered::<Registration>();
}

#[test]
fn registration_ids_support_diagnostic_interop() {
    assert_display::<RegistrationId>();
}

#[test]
fn wakers_expose_keys_and_cross_thread_traits() {
    let _ = Waker::key as fn(&Waker) -> Key;
    let _ = Waker::will_wake as fn(&Waker, &Waker) -> bool;
    let _ = Poll::waker_key as fn(&Poll) -> Option<Key>;
    assert_debug_send::<Poll>();
    assert_clone_debug_send_sync::<Waker>();
}
