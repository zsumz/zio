//! Registration ownership and retained-descriptor behavior.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use std::{io::Write, os::unix::net::UnixStream, time::Duration};

use zio::{Error, Event, Interest, Key, Mode, Poll, Wait};

#[test]
fn duplicate_descriptor_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = Poll::new()?;
    let registration = poll.register(&source, Key::new(1), Interest::READABLE, Mode::Level)?;

    for interest in [Interest::READABLE, Interest::WRITABLE] {
        let result = poll.register(&source, Key::new(2), interest, Mode::Level);
        assert!(matches!(
            result,
            Err(error)
                if matches!(
                    error.error(),
                    Error::Duplicate { existing, .. } if *existing == registration.id()
                )
        ));
    }

    poll.delete(registration)?;
    Ok(())
}

#[test]
fn registration_authority_is_poller_local() -> Result<(), Box<dyn std::error::Error>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut owner = Poll::new()?;
    let mut stranger = Poll::new()?;
    let registration = owner.register(&source, Key::new(7), Interest::READABLE, Mode::Level)?;

    let result = stranger.modify(
        &registration,
        Interest::READABLE | Interest::WRITABLE,
        Mode::Level,
    );
    assert!(matches!(result, Err(Error::WrongPoller { .. })));

    owner.delete(registration)?;
    Ok(())
}

#[test]
fn poll_retains_the_registered_open_file_description() -> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    let mut poll = Poll::new()?;
    let registration = poll.register(&source, Key::new(11), Interest::READABLE, Mode::Level)?;
    drop(source);

    peer.write_all(b"ready")?;
    let mut events = poll.events()?;
    poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    assert!(
        events
            .iter()
            .any(|event| { matches!(event, Event::Resource { key, .. } if *key == Key::new(11)) })
    );

    poll.delete(registration)?;
    Ok(())
}

#[test]
fn registration_capacity_is_fixed() -> Result<(), Box<dyn std::error::Error>> {
    let (first, _first_peer) = UnixStream::pair()?;
    let (second, _second_peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(4, 1)?;
    let registration = poll.register(&first, Key::new(1), Interest::READABLE, Mode::Level)?;

    let result = poll.register(&second, Key::new(2), Interest::READABLE, Mode::Level);
    assert!(matches!(
        result,
        Err(error) if matches!(error.error(), Error::Capacity { limit: 1 })
    ));

    poll.delete(registration)?;
    Ok(())
}
