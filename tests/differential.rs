//! Differential readiness checks against the temporary Mio oracle.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use std::{
    io::Write,
    os::{fd::AsRawFd, unix::net::UnixStream},
    time::Duration,
};

use mio::{Events as MioEvents, Interest as MioInterest, Poll as MioPoll, Token, unix::SourceFd};
use zio::{Event, Interest, Key, Mode, Poll, Wait};

#[test]
fn native_readiness_agrees_with_mio() -> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    source.set_nonblocking(true)?;
    let oracle_source = source.try_clone()?;
    let raw_descriptor = oracle_source.as_raw_fd();

    let mut poll = Poll::new()?;
    let registration = poll.register(&source, Key::new(17), Interest::READABLE, Mode::Level)?;
    let mut oracle = MioPoll::new()?;
    let mut oracle_registration = SourceFd(&raw_descriptor);
    oracle
        .registry()
        .register(&mut oracle_registration, Token(17), MioInterest::READABLE)?;

    peer.write_all(b"ready")?;
    let mut events = poll.events()?;
    poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    let mut oracle_events = MioEvents::with_capacity(8);
    oracle.poll(&mut oracle_events, Some(Duration::from_secs(1)))?;

    assert!(events.iter().any(|event| {
        matches!(event, Event::Resource { key, readiness }
            if *key == Key::new(17) && readiness.is_readable())
    }));
    assert!(
        oracle_events
            .iter()
            .any(|event| event.token() == Token(17) && event.is_readable())
    );

    poll.delete(registration)?;
    Ok(())
}
