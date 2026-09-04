//! Wrapped kqueue selection and recovery-order integration.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use std::{error::Error as StdError, num::NonZeroUsize, os::fd::AsFd, os::unix::net::UnixStream};

use crate::{
    ArmState, CommitStatus, Error, Event, Events, Interest, Key, Mode, Readiness, Registration,
    RegistrationId, RegistrationState, observe_recovery::DisarmOutcome,
    pending_kqueue::KqueuePending, table::RegistrationTable,
};

const ARMED: RegistrationState = RegistrationState::Registered {
    arm: ArmState::Armed,
};
const DISARMED: RegistrationState = RegistrationState::Registered {
    arm: ArmState::Disarmed,
};

#[test]
fn wrapped_selection_preserves_output_and_one_shot_disarm_order() -> Result<(), Box<dyn StdError>> {
    let capacity = NonZeroUsize::new(3).ok_or(Error::Invariant)?;
    let mut registrations = RegistrationTable::new(capacity)?;
    let (first_source, _first_peer) = UnixStream::pair()?;
    let (second_source, _second_peer) = UnixStream::pair()?;
    let (third_source, _third_peer) = UnixStream::pair()?;
    let first = reserve(&mut registrations, &first_source, Key::new(1))?;
    let second = reserve(&mut registrations, &second_source, Key::new(2))?;
    let third = reserve(&mut registrations, &third_source, Key::new(3))?;
    let mut pending = KqueuePending::new(capacity)?;
    add_all(&mut pending, [(first, 1), (second, 2), (third, 3)])?;
    let _ = pending.delivery_selection(2);
    pending.clear();
    add_all(&mut pending, [(first, 1), (second, 2), (third, 3)])?;
    let selection = pending.delivery_selection(2);
    let selected = selection.try_iter(pending.as_slice())?;
    let mut events = Events::with_capacity(2)?;

    crate::observe_recovery::finish(
        Some(owner()),
        &mut registrations,
        &mut events,
        selected,
        selection.len(),
        false,
        None,
        [
            outcome(first, CommitStatus::Applied),
            outcome(third, CommitStatus::Applied),
        ],
        None,
    )?
    .into_result()?;

    assert_eq!(
        events.as_slice(),
        &[
            Event::Resource {
                registration: handle(first),
                key: Key::new(1),
                readiness: Readiness::READABLE,
            },
            Event::Resource {
                registration: handle(third),
                key: Key::new(3),
                readiness: Readiness::READABLE,
            },
        ]
    );
    assert_eq!(registrations.state(first)?, DISARMED);
    assert_eq!(registrations.state(second)?, ARMED);
    assert_eq!(registrations.state(third)?, DISARMED);
    Ok(())
}

fn add_all(
    pending: &mut KqueuePending,
    registrations: [(RegistrationId, u64); 3],
) -> Result<(), Error> {
    for (registration, key) in registrations {
        pending.add(registration, Key::new(key), Readiness::READABLE)?;
    }
    Ok(())
}

fn reserve(
    registrations: &mut RegistrationTable,
    source: &UnixStream,
    key: Key,
) -> Result<RegistrationId, Box<dyn StdError>> {
    let descriptor = source.as_fd().try_clone_to_owned()?;
    Ok(registrations.reserve(descriptor, key, Interest::READABLE, Mode::OneShot)?)
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
