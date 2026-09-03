//! Downstream matching contracts for open diagnostics and closed domains.

use zio::{
    ArmState, CommitStatus, DeleteError, Error, Event, Key, Mode, Operation, Readiness,
    RecoveryOutcome, RegisterError, Registration, RegistrationState, Wait,
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
