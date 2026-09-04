//! Cross-poller event-destination reuse.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

mod support;

use std::{io::Write, os::unix::net::UnixStream, time::Duration};

use zio::{Event, Events, Interest, Key, Mode, Poll, Wait};

use support::require_no_recovery;

#[test]
fn larger_destination_is_reusable_across_pollers() -> Result<(), Box<dyn std::error::Error>> {
    let (first_source, mut first_peer) = UnixStream::pair()?;
    let (second_source, mut second_peer) = UnixStream::pair()?;
    let mut first_poll = Poll::with_capacity(1, 1)?;
    let mut second_poll = Poll::with_capacity(2, 1)?;
    let first = first_poll.register(&first_source, Key::new(1), Interest::READABLE, Mode::Level)?;
    let second =
        second_poll.register(&second_source, Key::new(2), Interest::READABLE, Mode::Level)?;
    let mut events = Events::with_capacity(2)?;

    first_peer.write_all(b"first")?;
    require_no_recovery(first_poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?)?;
    assert!(matches!(
        events.as_slice(),
        [Event::Resource { registration, key, .. }]
            if *registration == first && *key == Key::new(1)
    ));

    second_peer.write_all(b"second")?;
    require_no_recovery(second_poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?)?;
    assert!(matches!(
        events.as_slice(),
        [Event::Resource { registration, key, .. }]
            if *registration == second && *key == Key::new(2)
    ));

    first_poll.delete(first)?;
    second_poll.delete(second)?;
    Ok(())
}
