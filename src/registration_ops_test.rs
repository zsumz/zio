//! Lazy owner assignment through the public mutation surface.

use std::{
    error::Error as StdError,
    os::{
        fd::{AsFd, AsRawFd},
        unix::net::UnixStream,
    },
};

use crate::{
    ArmState, DescriptorOwnership, Error, Interest, Key, Mode, Poll, RegisterOwnedError,
    RegistrationState,
};

#[test]
fn registration_info_tracks_committed_configuration() -> Result<(), Box<dyn StdError>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(1, 1)?;
    let registration = poll.register(&source, Key::new(7), Interest::READABLE, Mode::Level)?;

    let initial = poll.registration_info(&registration)?;
    assert_eq!(initial.key(), Key::new(7));
    assert_eq!(initial.interest(), Interest::READABLE);
    assert_eq!(initial.mode(), Mode::Level);
    assert_eq!(initial.descriptor_ownership(), DescriptorOwnership::Owned);
    assert_eq!(
        initial.state(),
        RegistrationState::Registered {
            arm: ArmState::Armed,
        }
    );

    poll.modify(&registration, Interest::WRITABLE, Mode::OneShot)?;
    let modified = poll.registration_info(&registration)?;
    assert_eq!(modified.key(), Key::new(7));
    assert_eq!(modified.interest(), Interest::WRITABLE);
    assert_eq!(modified.mode(), Mode::OneShot);

    poll.delete(registration)?;
    assert!(matches!(
        poll.registration_info(&registration),
        Err(Error::Stale { registration: stale }) if stale == registration.id()
    ));
    Ok(())
}

#[test]
fn owned_registration_retains_the_transferred_descriptor() -> Result<(), Box<dyn StdError>> {
    let (source, _peer) = UnixStream::pair()?;
    let descriptor = source.as_fd().try_clone_to_owned()?;
    let raw = descriptor.as_raw_fd();
    let mut poll = Poll::with_capacity(1, 1)?;

    let registration =
        poll.register_owned(descriptor, Key::new(8), Interest::READABLE, Mode::Level)?;
    let retained = poll.registration_fd(&registration)?;

    assert_eq!(retained.as_raw_fd(), raw);
    poll.delete(registration)?;
    assert!(matches!(
        poll.registration_fd(&registration),
        Err(Error::Stale { registration: stale }) if stale == registration.id()
    ));
    Ok(())
}

#[test]
fn rejected_owned_registration_returns_the_transferred_descriptor() -> Result<(), Box<dyn StdError>>
{
    let (source, _peer) = UnixStream::pair()?;
    let descriptor = source.as_fd().try_clone_to_owned()?;
    let raw = descriptor.as_raw_fd();
    let mut poll = Poll::with_capacity(1, 1)?;

    let Err(RegisterOwnedError::Returned { error, descriptor }) =
        poll.register_owned(descriptor, Key::new(9), Interest::EMPTY, Mode::Level)
    else {
        return Err("empty interest did not return the owned descriptor".into());
    };

    assert!(matches!(error, Error::InvalidInterest));
    assert_eq!(descriptor.as_raw_fd(), raw);
    assert!(poll.owner.current().is_none());
    assert_eq!(poll.registration_count(), 0);
    Ok(())
}

#[test]
fn invalid_interest_leaves_owner_unassigned() -> Result<(), Box<dyn StdError>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(1, 1)?;

    let Err(error) = poll.register(&source, Key::new(1), Interest::EMPTY, Mode::Level) else {
        return Err("empty interest unexpectedly registered".into());
    };

    assert!(matches!(error.error(), Error::InvalidInterest));
    assert!(poll.owner.current().is_none());
    Ok(())
}

#[test]
fn wrong_poller_check_does_not_assign_stranger() -> Result<(), Box<dyn StdError>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut owner = Poll::with_capacity(1, 1)?;
    let mut stranger = Poll::with_capacity(1, 1)?;
    let registration = owner.register(&source, Key::new(2), Interest::READABLE, Mode::Level)?;

    let result = stranger.registration_state(&registration);

    assert!(matches!(result, Err(Error::WrongPoller { .. })));
    assert!(matches!(
        stranger.registration_fd(&registration),
        Err(Error::WrongPoller { registration: rejected }) if rejected == registration
    ));
    assert!(matches!(
        stranger.set_key(&registration, Key::new(3)),
        Err(Error::WrongPoller { registration: rejected }) if rejected == registration
    ));
    assert!(stranger.owner.current().is_none());
    Ok(())
}
