//! Atomic rejection evidence for malformed post-observation recovery plans.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use std::{error::Error as StdError, num::NonZeroUsize, os::fd::AsFd, os::unix::net::UnixStream};

use crate::{
    ArmState, CommitStatus, Error, Events, Interest, Key, Mode, Readiness, RecoveryOutcome,
    RegistrationId, RegistrationState, pending_kqueue::PendingResource, table::RegistrationTable,
};

const ARMED: RegistrationState = RegistrationState::Registered {
    arm: ArmState::Armed,
};
const DISARMED: RegistrationState = RegistrationState::Registered {
    arm: ArmState::Disarmed,
};

#[test]
fn extra_recovery_outcomes_are_rejected_atomically() -> Result<(), Box<dyn StdError>> {
    let mut registrations = table(2)?;
    let first = reserve(&mut registrations, Key::new(91))?;
    let extra = reserve(&mut registrations, Key::new(92))?;
    let pending = [pending(first, Key::new(91))];
    let outcomes = [
        outcome(first, CommitStatus::Applied),
        outcome(extra, CommitStatus::Applied),
    ];
    let mut events = Events::with_capacity(1)?;

    let result = finish(&mut registrations, &mut events, &pending, &outcomes);

    assert!(matches!(result, Err(Error::Invariant)));
    assert!(events.is_empty());
    assert_eq!(registrations.state(first)?, ARMED);
    assert_eq!(registrations.state(extra)?, ARMED);
    Ok(())
}

#[test]
fn reordered_recovery_outcomes_are_rejected_atomically() -> Result<(), Box<dyn StdError>> {
    let mut registrations = table(2)?;
    let first = reserve(&mut registrations, Key::new(93))?;
    let second = reserve(&mut registrations, Key::new(94))?;
    let pending = [pending(first, Key::new(93)), pending(second, Key::new(94))];
    let outcomes = [
        outcome(second, CommitStatus::Applied),
        outcome(first, CommitStatus::Applied),
    ];
    let mut events = Events::with_capacity(2)?;

    let result = finish(&mut registrations, &mut events, &pending, &outcomes);

    assert!(matches!(result, Err(Error::Invariant)));
    assert!(events.is_empty());
    assert_eq!(registrations.state(first)?, ARMED);
    assert_eq!(registrations.state(second)?, ARMED);
    Ok(())
}

#[test]
fn non_armed_recovery_plans_are_rejected_atomically() -> Result<(), Box<dyn StdError>> {
    assert_non_armed_plan(CommitStatus::Applied, DISARMED)?;
    assert_non_armed_plan(CommitStatus::Unknown, RegistrationState::Uncertain)
}

fn assert_non_armed_plan(
    established: CommitStatus,
    expected: RegistrationState,
) -> Result<(), Box<dyn StdError>> {
    let mut registrations = table(1)?;
    let registration = reserve(&mut registrations, Key::new(95))?;
    assert_eq!(
        registrations.apply_disarm(registration, established)?,
        expected
    );
    let pending = [pending(registration, Key::new(95))];
    let outcomes = [outcome(registration, CommitStatus::Applied)];
    let mut events = Events::with_capacity(1)?;

    let result = finish(&mut registrations, &mut events, &pending, &outcomes);

    assert!(matches!(result, Err(Error::Invariant)));
    assert!(events.is_empty());
    assert_eq!(registrations.state(registration)?, expected);
    Ok(())
}

fn finish(
    registrations: &mut RegistrationTable,
    events: &mut Events,
    pending: &[PendingResource],
    outcomes: &[RecoveryOutcome],
) -> Result<(), Error> {
    crate::observe_recovery::finish(
        registrations,
        events,
        pending,
        pending.len(),
        false,
        None,
        outcomes,
        None,
    )
}

fn table(capacity: usize) -> Result<RegistrationTable, Error> {
    RegistrationTable::new(NonZeroUsize::new(capacity).ok_or(Error::Invariant)?)
}

fn reserve(
    registrations: &mut RegistrationTable,
    key: Key,
) -> Result<RegistrationId, Box<dyn StdError>> {
    let (source, _peer) = UnixStream::pair()?;
    let descriptor = source.as_fd().try_clone_to_owned()?;
    Ok(registrations.reserve(descriptor, key, Interest::READABLE, Mode::OneShot)?)
}

const fn pending(registration: RegistrationId, key: Key) -> PendingResource {
    PendingResource {
        registration,
        key,
        readiness: Readiness::READABLE,
    }
}

const fn outcome(registration: RegistrationId, commit: CommitStatus) -> RecoveryOutcome {
    RecoveryOutcome::new(registration, commit)
}
