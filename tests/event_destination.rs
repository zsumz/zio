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

#[test]
fn drain_preserves_capacity_for_reuse() -> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(1, 1)?;
    let registration = poll.register(&source, Key::new(3), Interest::READABLE, Mode::Level)?;
    let mut events = poll.events()?;
    peer.write_all(b"ready")?;

    require_no_recovery(poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?)?;
    let mut drained = events.drain();
    assert!(matches!(
        drained.next_back(),
        Some(Event::Resource {
            registration: observed,
            key,
            ..
        }) if observed == registration && key == Key::new(3)
    ));
    assert!(drained.next().is_none());
    drop(drained);
    assert!(events.is_empty());
    assert_eq!(events.capacity(), 1);

    require_no_recovery(poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?)?;
    assert!(matches!(
        events.as_slice(),
        [Event::Resource {
            registration: observed,
            key,
            ..
        }] if *observed == registration && *key == Key::new(3)
    ));

    poll.delete(registration)?;
    Ok(())
}

#[test]
fn owned_iteration_preserves_delivery_order() -> Result<(), Box<dyn std::error::Error>> {
    let (first_source, mut first_peer) = UnixStream::pair()?;
    let (second_source, mut second_peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(2, 2)?;
    let first = poll.register(&first_source, Key::new(4), Interest::READABLE, Mode::Level)?;
    let second = poll.register(&second_source, Key::new(5), Interest::READABLE, Mode::Level)?;
    let mut events = poll.events()?;
    first_peer.write_all(b"first")?;
    second_peer.write_all(b"second")?;

    require_no_recovery(poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?)?;
    let delivered = events.as_slice().to_vec();
    assert_eq!(delivered.len(), 2);

    let mut owned = events.into_iter();
    assert_eq!(owned.len(), 2);
    assert_eq!(owned.next(), delivered.first().copied());
    assert_eq!(owned.next_back(), delivered.last().copied());
    assert_eq!(owned.next(), None);
    assert_eq!(owned.next_back(), None);

    poll.delete(first)?;
    poll.delete(second)?;
    Ok(())
}
