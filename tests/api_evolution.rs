//! Downstream matching contracts for open diagnostics and closed domains.

use core::{
    iter::FusedIterator,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Sub, SubAssign},
};

use zio::{
    ArmState, CommitStatus, DeleteError, Error, Event, Events, Key, Mode, MutationError, Operation,
    Poll, Readiness, RecoveryOutcome, RegisterError, Registration, RegistrationInfo,
    RegistrationState, Wait,
};

#[test]
fn open_diagnostics_support_forward_compatible_fallbacks() {
    assert_eq!(operation_class(Operation::Wait), "wait");
    assert_eq!(operation_class(Operation::Delete), "other");
    assert_eq!(error_class(&Error::Invariant), "contract");
    assert_eq!(error_class(&Error::UnsupportedPlatform), "other");
}

#[test]
fn closed_delivery_and_state_domains_remain_exhaustive() {
    assert_eq!(mode_class(Mode::Level), "level");
    assert_eq!(wait_class(Wait::NoBlock), "no-block");
    assert_eq!(commit_class(CommitStatus::Unknown), "unknown");
    assert_eq!(arm_class(ArmState::Disarmed), "disarmed");
    assert_eq!(state_class(RegistrationState::Uncertain), "uncertain");
}

#[test]
fn recovery_outcomes_return_registration_handles() {
    let _ = RecoveryOutcome::registration as fn(&RecoveryOutcome) -> Registration;
}

#[test]
fn errors_return_registration_handles() {
    let _ = RegisterError::registration as fn(&RegisterError) -> Option<Registration>;
    let _ = DeleteError::registration as fn(&DeleteError) -> Registration;
    let _ = rejected_registration as fn(&Error) -> Option<Registration>;
}

#[test]
fn mutation_errors_return_every_owned_detail() {
    let _ =
        MutationError::into_parts as fn(MutationError) -> (Operation, CommitStatus, std::io::Error);
}

#[test]
fn poll_exposes_stored_configuration_rearm() {
    let _ = Poll::rearm as fn(&mut Poll, &Registration) -> Result<(), Error>;
}

#[test]
fn poll_exposes_capacity_and_retained_count() {
    let _ = Poll::event_capacity as fn(&Poll) -> usize;
    let _ = Poll::registration_capacity as fn(&Poll) -> usize;
    let _ = Poll::registration_count as fn(&Poll) -> usize;
}

#[test]
fn poll_exposes_authoritative_registration_info() {
    let _ = Poll::registration_info as fn(&Poll, &Registration) -> Result<RegistrationInfo, Error>;
    let _ = RegistrationInfo::key as fn(&RegistrationInfo) -> Key;
    let _ = RegistrationInfo::interest as fn(&RegistrationInfo) -> zio::Interest;
    let _ = RegistrationInfo::mode as fn(&RegistrationInfo) -> Mode;
    let _ = RegistrationInfo::state as fn(&RegistrationInfo) -> RegistrationState;
    let _ = Poll::set_key as fn(&mut Poll, &Registration, Key) -> Result<(), Error>;
}

#[test]
fn flag_sets_support_standard_set_operators() {
    assert_set_operators::<zio::Interest>();
    assert_set_operators::<Readiness>();
}

#[test]
fn event_batches_support_immutable_slice_interop() {
    assert_event_slice::<Events>();
    let _ = assert_event_iterators as fn(&mut Events);
}

fn assert_event_slice<T: AsRef<[Event]>>() {}

fn assert_event_iterators(events: &mut Events) {
    assert_iterator(events.iter());
    assert_iterator(events.drain());
}

fn assert_iterator<I: DoubleEndedIterator + ExactSizeIterator + FusedIterator>(_: I) {}

fn assert_set_operators<T>()
where
    T: BitOr<Output = T>
        + BitOrAssign
        + BitAnd<Output = T>
        + BitAndAssign
        + Sub<Output = T>
        + SubAssign,
{
}

fn operation_class(operation: Operation) -> &'static str {
    match operation {
        Operation::Wait => "wait",
        _ => "other",
    }
}

fn error_class(error: &Error) -> &'static str {
    match error {
        Error::Invariant => "contract",
        _ => "other",
    }
}

fn rejected_registration(error: &Error) -> Option<Registration> {
    match error {
        Error::WrongPoller { registration } => Some(*registration),
        _ => None,
    }
}

fn mode_class(mode: Mode) -> &'static str {
    match mode {
        Mode::Level => "level",
        Mode::OneShot => "one-shot",
    }
}

fn wait_class(wait: Wait) -> &'static str {
    match wait {
        Wait::NoBlock => "no-block",
        Wait::For(_) => "for",
        Wait::Forever => "forever",
    }
}

fn commit_class(commit: CommitStatus) -> &'static str {
    match commit {
        CommitStatus::NotApplied => "not-applied",
        CommitStatus::Applied => "applied",
        CommitStatus::Unknown => "unknown",
    }
}

fn arm_class(arm: ArmState) -> &'static str {
    match arm {
        ArmState::Armed => "armed",
        ArmState::Disarmed => "disarmed",
    }
}

fn state_class(state: RegistrationState) -> &'static str {
    match state {
        RegistrationState::Registered { arm } => arm_class(arm),
        RegistrationState::Uncertain => "uncertain",
    }
}

#[allow(
    dead_code,
    reason = "compilation proves exhaustive downstream matching"
)]
fn event_class(event: Event) -> (Key, Option<Readiness>) {
    match event {
        Event::Resource { key, readiness, .. } => (key, Some(readiness)),
        Event::Wake { key, .. } => (key, None),
    }
}
