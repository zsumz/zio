//! Allocation contract regressions for post-observation kqueue recovery.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use std::{
    error::Error as StdError, io, mem::size_of, num::NonZeroUsize, os::fd::AsFd,
    os::unix::net::UnixStream,
};

use crate::{
    ArmState, CommitStatus, Error, Event, Events, Interest, Key, Mode, Readiness, RecoveryOutcome,
    RegistrationState, pending_kqueue::PendingResource, table::RegistrationTable,
};

#[test]
fn recovery_retains_exactly_one_bounded_allocation() -> Result<(), Box<dyn StdError>> {
    let capacity = NonZeroUsize::new(3).ok_or(Error::Invariant)?;
    let mut registrations = RegistrationTable::new(capacity)?;
    let (first_source, _first_peer) = UnixStream::pair()?;
    let (second_source, _second_peer) = UnixStream::pair()?;
    let (third_source, _third_peer) = UnixStream::pair()?;
    let first = reserve(&mut registrations, &first_source, Key::new(71))?;
    let second = reserve(&mut registrations, &second_source, Key::new(72))?;
    let third = reserve(&mut registrations, &third_source, Key::new(73))?;
    let pending = [
        pending(first, Key::new(71)),
        pending(second, Key::new(72)),
        pending(third, Key::new(73)),
    ];
    let outcomes = [
        RecoveryOutcome::new(first, CommitStatus::Applied),
        RecoveryOutcome::new(second, CommitStatus::NotApplied),
        RecoveryOutcome::new(third, CommitStatus::Unknown),
    ];
    let mut events = Events::with_capacity(3)?;
    let native_error = io::Error::from_raw_os_error(5);
    let mut result = None;

    let allocations = allocation_counter::measure(|| {
        result = Some(crate::observe_recovery::finish(
            &mut registrations,
            &mut events,
            &pending,
            pending.len(),
            false,
            None,
            outcomes,
            Some(native_error),
        ));
    });

    let failure = match result {
        Some(Err(Error::Recovery(failure))) => failure,
        Some(Err(other)) => return Err(Box::new(other)),
        Some(Ok(())) => return Err(io::Error::other("recovery unexpectedly succeeded").into()),
        None => return Err(io::Error::other("measured call did not complete").into()),
    };
    let snapshot_bytes = u64::try_from(size_of::<RecoveryOutcome>() * outcomes.len())?;
    assert_eq!(allocations.count_total, 1);
    assert_eq!(allocations.count_current, 1);
    assert_eq!(allocations.count_max, 1);
    assert_eq!(allocations.bytes_total, snapshot_bytes);
    assert_eq!(allocations.bytes_current, i64::try_from(snapshot_bytes)?);
    assert_eq!(allocations.bytes_max, snapshot_bytes);
    assert_eq!(failure.outcomes(), outcomes);
    assert_eq!(
        registrations.state(first)?,
        RegistrationState::Registered {
            arm: ArmState::Disarmed,
        }
    );
    assert_eq!(
        registrations.state(second)?,
        RegistrationState::Registered {
            arm: ArmState::Armed,
        }
    );
    assert_eq!(registrations.state(third)?, RegistrationState::Uncertain);
    assert_eq!(
        events.as_slice(),
        &[
            Event::Resource {
                key: Key::new(71),
                readiness: Readiness::READABLE,
            },
            Event::Resource {
                key: Key::new(72),
                readiness: Readiness::READABLE,
            },
            Event::Resource {
                key: Key::new(73),
                readiness: Readiness::READABLE,
            },
        ]
    );
    Ok(())
}

fn reserve(
    registrations: &mut RegistrationTable,
    source: &UnixStream,
    key: Key,
) -> Result<crate::RegistrationId, Box<dyn StdError>> {
    let descriptor = source.as_fd().try_clone_to_owned()?;
    Ok(registrations.reserve(descriptor, key, Interest::READABLE, Mode::OneShot)?)
}

const fn pending(registration: crate::RegistrationId, key: Key) -> PendingResource {
    PendingResource {
        registration,
        key,
        readiness: Readiness::READABLE,
    }
}
