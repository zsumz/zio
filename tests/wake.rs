//! Cross-thread wake lifecycle behavior.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

mod support;

use std::{collections::HashSet, io, io::Write, os::unix::net::UnixStream, thread, time::Duration};

use zio::{Event, Interest, Key, Mode, Poll, Wait};

use support::require_no_recovery;

#[test]
fn wake_before_wait_is_observable() -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = Poll::new()?;
    assert_eq!(poll.waker_key(), None);
    let waker = poll.waker(Key::new(99))?;
    assert_eq!(poll.waker_key(), Some(Key::new(99)));
    waker.wake()?;

    let mut events = poll.events()?;
    let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    assert!(matches!(
        events.as_slice(),
        [Event::Wake { key, .. }] if *key == Key::new(99)
    ));
    require_no_recovery(report)?;
    Ok(())
}

#[test]
fn wake_coalesces_drains_and_remains_reusable() -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let key = Key::new(101);
    let waker = poll.waker(key)?;
    let mut events = poll.events()?;

    waker.wake()?;
    waker.wake()?;
    require_no_recovery(poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?)?;
    assert!(has_wake(&events, key));

    require_no_recovery(poll.wait(&mut events, Wait::NoBlock)?)?;
    assert!(events.is_empty());

    waker.wake()?;
    require_no_recovery(poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?)?;
    assert!(has_wake(&events, key));
    Ok(())
}

#[test]
fn wake_and_resource_share_capacity_without_loss() -> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    source.set_nonblocking(true)?;
    let mut poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let resource_key = Key::new(102);
    let wake_key = Key::new(103);
    let registration = poll.register(&source, resource_key, Interest::READABLE, Mode::OneShot)?;
    let waker = poll.waker(wake_key)?;
    let mut events = poll.events()?;
    peer.write_all(b"ready")?;
    waker.wake()?;

    let mut saw_resource = false;
    let mut saw_wake = false;
    for _ in 0..2 {
        require_no_recovery(poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?)?;
        match events.as_slice() {
            [
                Event::Resource {
                    registration: actual,
                    key,
                    ..
                },
            ] if *actual == registration && *key == resource_key => saw_resource = true,
            [Event::Wake { key, .. }] if *key == wake_key => saw_wake = true,
            observed => {
                return Err(io::Error::other(format!("unexpected events: {observed:?}")).into());
            }
        }
    }

    assert!(saw_resource);
    assert!(saw_wake);
    poll.delete(registration)?;
    Ok(())
}

#[test]
fn repeated_wakes_do_not_starve_a_ready_resource() -> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    source.set_nonblocking(true)?;
    let mut poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let resource_key = Key::new(104);
    let registration = poll.register(&source, resource_key, Interest::READABLE, Mode::Level)?;
    let waker = poll.waker(Key::new(105))?;
    let mut events = poll.events()?;
    peer.write_all(b"ready")?;

    let mut observed = false;
    for _ in 0..4 {
        waker.wake()?;
        require_no_recovery(poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?)?;
        observed = events.iter().any(|event| {
            matches!(
                event,
                Event::Resource {
                    registration: actual,
                    key,
                    ..
                } if *actual == registration && *key == resource_key
            )
        });
        if observed {
            break;
        }
    }

    assert!(observed, "repeated wakes starved a ready resource");
    poll.delete(registration)?;
    Ok(())
}

#[test]
fn repeated_wakes_preserve_ready_set_fairness() -> Result<(), Box<dyn std::error::Error>> {
    verify_wake_pressure_fairness(Mode::Level)?;
    verify_wake_pressure_fairness(Mode::OneShot)
}

#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
#[test]
fn repeated_wakes_do_not_shrink_wrapped_resource_batches() -> Result<(), Box<dyn std::error::Error>>
{
    let mut poll = Poll::builder()
        .event_capacity(2)
        .registration_capacity(3)
        .build()?;
    let mut sources = Vec::new();
    let mut peers = Vec::new();
    for key in [Key::new(120), Key::new(121), Key::new(122)] {
        let (source, mut peer) = UnixStream::pair()?;
        source.set_nonblocking(true)?;
        peer.write_all(b"ready")?;
        let _registration = poll.register(&source, key, Interest::READABLE, Mode::Level)?;
        sources.push(source);
        peers.push(peer);
    }
    let wake_key = Key::new(123);
    let waker = poll.waker(wake_key)?;
    let mut events = poll.events()?;
    let mut full_resource_batches = 0;
    let mut saw_wake = false;

    for _ in 0..8 {
        waker.wake()?;
        require_no_recovery(poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?)?;
        let resources = events.iter().filter(|event| event.is_resource()).count();
        if resources != 0 {
            assert_eq!(resources, 2);
            full_resource_batches += 1;
        }
        saw_wake |= events
            .iter()
            .any(|event| event.is_wake() && event.key() == wake_key);
    }

    assert!(full_resource_batches >= 3);
    assert!(saw_wake);
    poll.delete_all()?;
    drop((sources, peers));
    Ok(())
}

fn verify_wake_pressure_fairness(mode: Mode) -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(3)
        .build()?;
    let mut sources = Vec::new();
    let mut peers = Vec::new();
    let mut registrations = Vec::new();
    let keys = [Key::new(110), Key::new(111), Key::new(112)];
    let expected = HashSet::from(keys);
    for key in keys {
        let (source, mut peer) = UnixStream::pair()?;
        source.set_nonblocking(true)?;
        peer.write_all(b"ready")?;
        registrations.push(poll.register(&source, key, Interest::READABLE, mode)?);
        sources.push(source);
        peers.push(peer);
    }
    let wake_key = Key::new(113);
    let waker = poll.waker(wake_key)?;
    let mut events = poll.events()?;
    let mut seen = HashSet::new();
    let mut saw_wake = false;

    for _ in 0..12 {
        waker.wake()?;
        require_no_recovery(poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?)?;
        for event in &events {
            match event {
                Event::Resource { key, .. } => {
                    seen.insert(*key);
                }
                Event::Wake { key, .. } => saw_wake |= *key == wake_key,
            }
        }
        if seen == expected && saw_wake {
            break;
        }
    }

    assert_eq!(seen, expected);
    assert_eq!(seen.len(), registrations.len());
    assert!(saw_wake);
    poll.delete_all()?;
    drop((sources, peers));
    Ok(())
}

#[test]
fn wake_completes_a_blocked_wait() -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = Poll::new()?;
    let waker = poll.waker(Key::new(100))?;
    let thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        waker.wake()
    });

    let mut events = poll.events()?;
    let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    let wake_result = thread
        .join()
        .map_err(|_| io::Error::other("wake thread panicked"))?;
    wake_result?;
    assert!(has_wake(&events, Key::new(100)));
    require_no_recovery(report)?;
    Ok(())
}

fn has_wake(events: &zio::Events, expected: Key) -> bool {
    matches!(events.as_slice(), [Event::Wake { key, .. }] if *key == expected)
}
