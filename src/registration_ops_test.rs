//! Lazy owner assignment through the public mutation surface.

use std::{error::Error as StdError, os::unix::net::UnixStream};

use crate::{Error, Interest, Key, Mode, Poll};

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
