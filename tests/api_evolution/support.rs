//! Trait assertions and event-iterator probes.

use core::{
    fmt::Debug,
    hash::Hash,
    iter::FusedIterator,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Sub, SubAssign},
};

use zio::{
    ArmState, CapacityKind, CapacityReason, CommitStatus, DeleteOwnedError, DescriptorOwnership,
    Error, Event, Events, Key, Mode, Operation, Readiness, RecoveryFailure, RecoveryOutcome,
    RegisterOwnedError, RegistrationState, Wait,
};

pub(super) fn operation_class(operation: Operation) -> &'static str {
    match operation {
        Operation::Wait => "wait",
        _ => "other",
    }
}

pub(super) fn error_class(error: &Error) -> &'static str {
    match error {
        Error::Invariant => "contract",
        _ => "other",
    }
}

pub(super) fn capacity_kind_class(kind: CapacityKind) -> &'static str {
    match kind {
        CapacityKind::Event => "event",
        CapacityKind::Registration => "registration",
        _ => "other",
    }
}

pub(super) fn capacity_reason_class(reason: CapacityReason) -> &'static str {
    match reason {
        CapacityReason::Zero => "zero",
        CapacityReason::BackendLimit => "backend-limit",
        CapacityReason::Exhausted => "exhausted",
        CapacityReason::GenerationExhausted => "generation-exhausted",
        CapacityReason::StorageUnavailable => "storage",
        _ => "other",
    }
}

pub(super) fn mode_class(mode: Mode) -> &'static str {
    match mode {
        Mode::Level => "level",
        Mode::OneShot => "one-shot",
    }
}

pub(super) fn wait_class(wait: Wait) -> &'static str {
    match wait {
        Wait::NoBlock => "no-block",
        Wait::For(_) => "for",
        Wait::Forever => "forever",
    }
}

pub(super) fn commit_class(commit: CommitStatus) -> &'static str {
    match commit {
        CommitStatus::NotApplied => "not-applied",
        CommitStatus::Applied => "applied",
        CommitStatus::Unknown => "unknown",
    }
}

pub(super) fn arm_class(arm: ArmState) -> &'static str {
    match arm {
        ArmState::Armed => "armed",
        ArmState::Disarmed => "disarmed",
    }
}

pub(super) fn state_class(state: RegistrationState) -> &'static str {
    match state {
        RegistrationState::Registered { arm } => arm_class(arm),
        RegistrationState::Uncertain => "uncertain",
    }
}

pub(super) fn ownership_class(ownership: DescriptorOwnership) -> &'static str {
    match ownership {
        DescriptorOwnership::Owned => "owned",
        DescriptorOwnership::Borrowed => "borrowed",
    }
}

pub(super) fn owned_register_error_class(error: &RegisterOwnedError) -> &'static str {
    match error {
        RegisterOwnedError::Returned { .. } => "descriptor",
        RegisterOwnedError::Retained { .. } => "registration",
    }
}

pub(super) fn owned_delete_error_class(error: &DeleteOwnedError) -> &'static str {
    match error {
        DeleteOwnedError::Returned { .. } => "descriptor",
        DeleteOwnedError::Retained { .. } => "registration",
    }
}

#[allow(
    dead_code,
    reason = "compilation proves exhaustive downstream matching"
)]
pub(super) fn event_class(event: Event) -> (Key, Option<Readiness>) {
    match event {
        Event::Resource { key, readiness, .. } => (key, Some(readiness)),
        Event::Wake { key, .. } => (key, None),
    }
}

pub(super) fn assert_slice<T: AsRef<[U]>, U>() {}

pub(super) fn assert_ordered<T: Ord>() {}

pub(super) fn assert_hash<T: core::hash::Hash>() {}

pub(super) fn assert_display<T: core::fmt::Display>() {}

pub(super) fn assert_error_ref<T: AsRef<Error>>() {}

pub(super) fn assert_thread_error<T: std::error::Error + Send + Sync + 'static>() {}

pub(super) fn assert_debug_thread_value<T: Debug + Send + Sync + 'static>() {}

pub(super) fn assert_copy_value<T: Copy + Debug + Eq + Hash + Send + Sync + 'static>() {}

pub(super) fn assert_debug_send<T: Debug + Send>() {}

pub(super) fn assert_clone_debug_send_sync<T: Clone + Debug + Send + Sync>() {}

pub(super) fn assert_from<T: From<U>, U>() {}

pub(super) fn assert_event_iterators(events: &mut Events) {
    assert_iterator(events.iter());
    assert_iterator(events.drain());
}

pub(super) fn assert_owned_event_iterator(events: Events) {
    assert_iterator(events.into_iter());
}

pub(super) fn assert_recovery_iterator(failure: &RecoveryFailure) {
    assert_iterator(failure.iter());
    assert_iterator(failure.into_iter());
    let _: Option<&RecoveryOutcome> = failure.into_iter().next();
}

pub(super) fn assert_registration_iterator(poll: &zio::Poll) -> Result<(), Error> {
    assert_iterator(poll.iter_registrations()?);
    Ok(())
}

fn assert_iterator<I: DoubleEndedIterator + ExactSizeIterator + FusedIterator>(_: I) {}

pub(super) fn assert_set_operators<T>()
where
    T: BitOr<Output = T>
        + BitOrAssign
        + BitAnd<Output = T>
        + BitAndAssign
        + BitXor<Output = T>
        + BitXorAssign
        + Not<Output = T>
        + Sub<Output = T>
        + SubAssign,
{
}
