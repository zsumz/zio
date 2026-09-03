//! Lazy owner assignment through the public mutation surface.

use std::{error::Error as StdError, os::unix::net::UnixStream};

use crate::{ArmState, Error, Interest, Key, Mode, Poll, RegistrationState};

#[test]
fn registration_info_tracks_committed_configuration() -> Result<(), Box<dyn StdError>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(1, 1)?;
    let registration = poll.register(&source, Key::new(7), Interest::READABLE, Mode::Level)?;

    let initial = poll.registration_info(&registration)?;
    assert_eq!(initial.key(), Key::new(7));
    assert_eq!(initial.interest(), Interest::READABLE);
    assert_eq!(initial.mode(), Mode::Level);
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
    let stranger = Poll::with_capacity(1, 1)?;
    let registration = owner.register(&source, Key::new(2), Interest::READABLE, Mode::Level)?;

    let result = stranger.registration_state(&registration);

    assert!(matches!(result, Err(Error::WrongPoller { .. })));
    assert!(stranger.owner.current().is_none());
    Ok(())
}
