//! Post-observation recovery contract regressions.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use std::{
    error::Error as StdError, io, num::NonZeroUsize, os::fd::AsFd, os::unix::net::UnixStream,
};

use crate::{
    ArmState, CommitStatus, Error, Event, Events, Interest, Key, Mode, Readiness, RecoveryFailure,
    Registration, RegistrationId, RegistrationState, WaitReport, observe_recovery::DisarmOutcome,
    pending_kqueue::PendingResource, table::RegistrationTable,
};

const ARMED: RegistrationState = RegistrationState::Registered {
    arm: ArmState::Armed,
};
const DISARMED: RegistrationState = RegistrationState::Registered {
    arm: ArmState::Disarmed,
};

#[test]
fn recovery_failure_preserves_translated_resource_and_wake_events() -> Result<(), Box<dyn StdError>>
{
    let mut registrations = table(3)?;
    let (first, _first_peer) = UnixStream::pair()?;
    let (second, _second_peer) = UnixStream::pair()?;
    let (third, _third_peer) = UnixStream::pair()?;
    let applied = reserve(&mut registrations, &first, Key::new(11))?;
    let not_applied = reserve(&mut registrations, &second, Key::new(12))?;
    let unknown = reserve(&mut registrations, &third, Key::new(13))?;
    let pending = [
        pending(applied, Key::new(11), Readiness::READABLE),
        pending(not_applied, Key::new(12), Readiness::WRITABLE),
        pending(unknown, Key::new(13), Readiness::READABLE),
    ];
    let outcomes = [
        outcome(applied, CommitStatus::Applied),
        outcome(not_applied, CommitStatus::NotApplied),
        outcome(unknown, CommitStatus::Unknown),
    ];
    let mut events = Events::with_capacity(4)?;

    let result = crate::observe_recovery::finish(
        Some(owner()),
        &mut registrations,
        &mut events,
        &pending,
        pending.len(),
        true,
        Some(Key::new(99)),
        outcomes,
        Some(io::Error::from_raw_os_error(5)),
    );
    let failure = recovery(result)?;

    assert_eq!(
        events.as_slice(),
        &[
            Event::Resource {
                registration: handle(applied),
                key: Key::new(11),
                readiness: Readiness::READABLE,
            },
            Event::Resource {
                registration: handle(not_applied),
                key: Key::new(12),
                readiness: Readiness::WRITABLE,
            },
            Event::Resource {
                registration: handle(unknown),
                key: Key::new(13),
                readiness: Readiness::READABLE,
            },
            Event::Wake { key: Key::new(99) },
        ]
    );
    assert_eq!(failure.outcomes().len(), 3);
    assert_eq!(failure.outcomes()[0].registration(), handle(applied));
    assert_eq!(failure.outcomes()[1].registration(), handle(not_applied));
    assert_eq!(failure.outcomes()[2].registration(), handle(unknown));
    assert_eq!(failure.outcomes()[0].commit(), CommitStatus::Applied);
    assert_eq!(failure.outcomes()[0].state(), DISARMED);
    assert_eq!(failure.outcomes()[1].state(), ARMED);
    assert_eq!(failure.outcomes()[2].state(), RegistrationState::Uncertain);
    assert_eq!(failure.source().raw_os_error(), Some(5));
    assert_eq!(registrations.state(applied)?, DISARMED);
    assert_eq!(registrations.state(not_applied)?, ARMED);
    assert_eq!(registrations.state(unknown)?, RegistrationState::Uncertain);
    let (_, snapshot, source) = failure.into_parts();
    assert_eq!(snapshot.len(), outcomes.len());
    assert_eq!(source.raw_os_error(), Some(5));
    Ok(())
}

#[test]
fn retained_recovery_reports_survive_poll_state_reuse() -> Result<(), Box<dyn StdError>> {
    let mut registrations = table(1)?;
    let (source, _peer) = UnixStream::pair()?;
    let registration = reserve(&mut registrations, &source, Key::new(41))?;
    let pending = [pending(registration, Key::new(41), Readiness::READABLE)];

    let first = finish_round(
        &mut registrations,
        &pending,
        registration,
        CommitStatus::NotApplied,
        Some(5),
    )?;
    let second = finish_round(
        &mut registrations,
        &pending,
        registration,
        CommitStatus::NotApplied,
        Some(6),
    )?;
    let success = finish_round(
        &mut registrations,
        &pending,
        registration,
        CommitStatus::Applied,
        None,
    )?;

    let first = first.ok_or_else(|| io::Error::other("first recovery unexpectedly succeeded"))?;
    let second =
        second.ok_or_else(|| io::Error::other("second recovery unexpectedly succeeded"))?;
    assert!(success.is_none());
    assert_ne!(first.outcomes().as_ptr(), second.outcomes().as_ptr());
    assert_eq!(first.outcomes()[0].commit(), CommitStatus::NotApplied);
    assert_eq!(first.source().raw_os_error(), Some(5));
    assert_eq!(second.outcomes()[0].commit(), CommitStatus::NotApplied);
    assert_eq!(second.source().raw_os_error(), Some(6));
    assert_eq!(first.outcomes()[0].registration(), handle(registration));
    assert_eq!(second.outcomes()[0].registration(), handle(registration));
    assert_eq!(first.outcomes()[0].state(), ARMED);
    assert_eq!(second.outcomes()[0].state(), ARMED);
    assert_eq!(registrations.state(registration)?, DISARMED);
    Ok(())
}

#[test]
fn incomplete_recovery_outcomes_fail_before_observation_or_state_change()
-> Result<(), Box<dyn StdError>> {
    let mut registrations = table(2)?;
    let (first, _first_peer) = UnixStream::pair()?;
    let (second, _second_peer) = UnixStream::pair()?;
    let first_id = reserve(&mut registrations, &first, Key::new(51))?;
    let second_id = reserve(&mut registrations, &second, Key::new(52))?;
    let pending = [
        pending(first_id, Key::new(51), Readiness::READABLE),
        pending(second_id, Key::new(52), Readiness::READABLE),
    ];
    let mut events = Events::with_capacity(2)?;

    let result = crate::observe_recovery::finish(
        Some(owner()),
        &mut registrations,
        &mut events,
        &pending,
        pending.len(),
        false,
        None,
        [outcome(first_id, CommitStatus::Applied)],
        None,
    );

    assert!(matches!(result, Err(Error::Invariant)));
    assert!(events.is_empty());
    assert_armed(&registrations, first_id)?;
    assert_armed(&registrations, second_id)?;
    Ok(())
}

#[test]
fn recovery_source_presence_matches_degraded_outcomes() -> Result<(), Box<dyn StdError>> {
    for (commit, error) in [
        (CommitStatus::Applied, Some(5)),
        (CommitStatus::NotApplied, None),
    ] {
        let mut registrations = table(1)?;
        let (source, _peer) = UnixStream::pair()?;
        let registration = reserve(&mut registrations, &source, Key::new(61))?;
        let pending = [pending(registration, Key::new(61), Readiness::READABLE)];
        let mut events = Events::with_capacity(1)?;
        let result = crate::observe_recovery::finish(
            Some(owner()),
            &mut registrations,
            &mut events,
            &pending,
            1,
            false,
            None,
            [outcome(registration, commit)],
            error.map(io::Error::from_raw_os_error),
        );
        assert!(matches!(result, Err(Error::Invariant)));
        assert!(events.is_empty());
        assert_armed(&registrations, registration)?;
    }
    Ok(())
}

fn finish_round(
    registrations: &mut RegistrationTable,
    pending: &[PendingResource],
    registration: RegistrationId,
    commit: CommitStatus,
    error: Option<i32>,
) -> Result<Option<RecoveryFailure>, Box<dyn StdError>> {
    let mut events = Events::with_capacity(1)?;
    let result = crate::observe_recovery::finish(
        Some(owner()),
        registrations,
        &mut events,
        pending,
        1,
        false,
        None,
        [outcome(registration, commit)],
        error.map(io::Error::from_raw_os_error),
    );
    assert_eq!(
        events.as_slice(),
        &[Event::Resource {
            registration: handle(registration),
            key: Key::new(41),
            readiness: Readiness::READABLE,
        }]
    );
    Ok(result?.into_recovery())
}

fn recovery(result: Result<WaitReport, Error>) -> Result<RecoveryFailure, Box<dyn StdError>> {
    result?
        .into_recovery()
        .ok_or_else(|| Box::new(io::Error::other("recovery unexpectedly succeeded")) as _)
}

fn table(capacity: usize) -> Result<RegistrationTable, Error> {
    let capacity = NonZeroUsize::new(capacity).ok_or(Error::Invariant)?;
    RegistrationTable::new(capacity)
}

fn reserve(
    registrations: &mut RegistrationTable,
    source: &UnixStream,
    key: Key,
) -> Result<RegistrationId, Box<dyn StdError>> {
    let descriptor = source.as_fd().try_clone_to_owned()?;
    Ok(registrations.reserve(descriptor, key, Interest::READABLE, Mode::OneShot)?)
}

fn assert_armed(table: &RegistrationTable, id: RegistrationId) -> Result<(), Error> {
    assert_eq!(table.state(id)?, ARMED);
    Ok(())
}

const fn pending(registration: RegistrationId, key: Key, readiness: Readiness) -> PendingResource {
    PendingResource {
        registration,
        key,
        readiness,
    }
}

const fn handle(registration: RegistrationId) -> Registration {
    Registration::test(registration.get())
}

const fn owner() -> crate::registration::PollId {
    Registration::test(1).owner()
}

const fn outcome(registration: RegistrationId, commit: CommitStatus) -> DisarmOutcome {
    DisarmOutcome::new(registration, commit)
}
