//! Lazy poll-owner identity regressions.

use std::{
    fs::File,
    num::NonZeroUsize,
    os::fd::AsFd,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{Error, Interest, Key, Mode, registration::PollOwner, table::RegistrationTable};

#[test]
fn owner_is_allocated_once() -> Result<(), Error> {
    let next = AtomicU64::new(41);
    let mut owner = PollOwner::unassigned();

    let first = owner.get_or_assign_from(&next)?;
    let second = owner.get_or_assign_from(&next)?;

    assert_eq!(first, second);
    assert_eq!(next.load(Ordering::Relaxed), 42);
    Ok(())
}

#[test]
fn identity_failure_leaves_owner_unassigned() {
    let next = AtomicU64::new(u64::MAX);
    let mut owner = PollOwner::unassigned();

    assert!(matches!(
        owner.get_or_assign_from(&next),
        Err(Error::Invariant)
    ));
    assert!(owner.current().is_none());
    assert_eq!(next.load(Ordering::Relaxed), u64::MAX);
}

#[test]
fn capacity_preflight_does_not_attempt_identity_assignment()
-> Result<(), Box<dyn std::error::Error>> {
    let mut table = RegistrationTable::new(NonZeroUsize::new(1).ok_or(Error::Invariant)?)?;
    let source = File::open("/dev/null")?;
    let descriptor = source.as_fd().try_clone_to_owned()?;
    table.reserve(descriptor, Key::new(1), Interest::READABLE, Mode::Level)?;
    let next = AtomicU64::new(u64::MAX);
    let mut owner = PollOwner::unassigned();

    let result = table.check_reservable();
    if result.is_ok() {
        owner.get_or_assign_from(&next)?;
    }

    assert!(matches!(result, Err(Error::Capacity { limit: 1 })));
    assert!(owner.current().is_none());
    assert_eq!(next.load(Ordering::Relaxed), u64::MAX);
    Ok(())
}

#[test]
fn identity_failure_does_not_consume_a_registration_generation()
-> Result<(), Box<dyn std::error::Error>> {
    let capacity = NonZeroUsize::new(1).ok_or(Error::Invariant)?;
    let mut attempted = RegistrationTable::new(capacity)?;
    let mut baseline = RegistrationTable::new(capacity)?;
    let next = AtomicU64::new(u64::MAX);
    let mut owner = PollOwner::unassigned();

    attempted.check_reservable()?;
    assert!(matches!(
        owner.get_or_assign_from(&next),
        Err(Error::Invariant)
    ));

    let source = File::open("/dev/null")?;
    let attempted_id = attempted.reserve(
        source.as_fd().try_clone_to_owned()?,
        Key::new(1),
        Interest::READABLE,
        Mode::Level,
    )?;
    let baseline_id = baseline.reserve(
        source.as_fd().try_clone_to_owned()?,
        Key::new(1),
        Interest::READABLE,
        Mode::Level,
    )?;

    assert_eq!(attempted_id, baseline_id);
    Ok(())
}
