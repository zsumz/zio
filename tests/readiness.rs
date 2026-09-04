//! Readiness, one-shot, and wake behavior on supported Unix hosts.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

mod support;

use std::{
    io::{self, Write},
    os::unix::net::UnixStream,
    thread,
    time::{Duration, Instant},
};

use zio::{
    ArmState, CapacityKind, CapacityReason, Error, Event, Events, Interest, Key, Mode, Poll,
    RegistrationState, Wait,
};

use support::require_no_recovery;

#[test]
fn one_shot_requires_explicit_rearm() -> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    let mut poll = Poll::new()?;
    let registration = poll.register(&source, Key::new(41), Interest::READABLE, Mode::OneShot)?;
    peer.write_all(b"still readable")?;

    let mut events = poll.events()?;
    let report = poll.wait_until(&mut events, Instant::now() + Duration::from_secs(1))?;
    assert!(has_key(&events, Key::new(41)));
    let delivered = events
        .get(0)
        .and_then(|event| event.registration())
        .ok_or_else(|| io::Error::other("missing resource registration"))?;
    assert_eq!(delivered, registration);
    require_no_recovery(report)?;
    assert_eq!(
        poll.registration_state(&registration)?,
        RegistrationState::Registered {
            arm: ArmState::Disarmed,
        }
    );

    let report = poll.wait(&mut events, Wait::NoBlock)?;
    assert!(events.is_empty());
    require_no_recovery(report)?;

    poll.rearm(&delivered)?;
    assert_eq!(
        poll.registration_state(&registration)?,
        RegistrationState::Registered {
            arm: ArmState::Armed,
        }
    );
    let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    assert!(has_key(&events, Key::new(41)));
    require_no_recovery(report)?;

    poll.delete(delivered)?;
    Ok(())
}

#[test]
fn reached_deadline_is_nonblocking() -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = Poll::new()?;
    let mut events = poll.events()?;

    let report = poll.wait_until(&mut events, Instant::now())?;

    assert!(events.is_empty());
    require_no_recovery(report)?;
    Ok(())
}

#[test]
fn future_deadline_wait_is_wakeable() -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = Poll::new()?;
    let key = Key::new(43);
    let waker = poll.waker(key)?;
    let thread = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        waker.wake()
    });
    let mut events = poll.events()?;

    let report = poll.wait_until(&mut events, Instant::now() + Duration::from_secs(1))?;
    let wake_result = thread
        .join()
        .map_err(|_| io::Error::other("wake thread panicked"))?;

    wake_result?;
    require_no_recovery(report)?;
    assert!(matches!(
        events.as_slice(),
        [Event::Wake { key: actual, .. }] if *actual == key
    ));
    Ok(())
}

#[test]
fn duplicate_keys_preserve_registration_identity() -> Result<(), Box<dyn std::error::Error>> {
    let (first, mut first_peer) = UnixStream::pair()?;
    let (second, mut second_peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(2, 2)?;
    let key = Key::new(42);
    let first_registration = poll.register(&first, key, Interest::READABLE, Mode::Level)?;
    let second_registration = poll.register(&second, key, Interest::READABLE, Mode::Level)?;
    first_peer.write_all(b"first")?;
    second_peer.write_all(b"second")?;

    let mut events = poll.events()?;
    let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    require_no_recovery(report)?;

    assert_eq!(events.len(), 2);
    for registration in [first_registration, second_registration] {
        assert!(events.iter().any(|event| {
            matches!(
                event,
                Event::Resource {
                    registration: actual,
                    key: actual_key,
                    readiness,
                    ..
                } if *actual == registration && *actual_key == key && readiness.is_readable()
            )
        }));
    }

    poll.delete(first_registration)?;
    poll.delete(second_registration)?;
    Ok(())
}

#[test]
fn one_shot_coalesces_readable_and_writable_at_capacity_one()
-> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(1, 1)?;
    let registration = poll.register(
        &source,
        Key::new(42),
        Interest::READABLE | Interest::WRITABLE,
        Mode::OneShot,
    )?;
    peer.write_all(b"readable and writable")?;

    let mut events = poll.events()?;
    assert!(!events.is_full());
    assert_eq!(events.remaining_capacity(), 1);
    let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    let [Event::Resource { key, readiness, .. }] = events.as_slice() else {
        return Err(io::Error::other("expected one coalesced resource event").into());
    };
    assert!(events.is_full());
    assert_eq!(events.remaining_capacity(), 0);
    assert_eq!(*key, Key::new(42));
    assert!(readiness.contains(zio::Readiness::READABLE.union(zio::Readiness::WRITABLE)));
    require_no_recovery(report)?;
    assert_eq!(
        poll.registration_state(&registration)?,
        RegistrationState::Registered {
            arm: ArmState::Disarmed,
        }
    );

    events.clear();
    assert!(!events.is_full());
    assert_eq!(events.remaining_capacity(), 1);

    poll.delete(registration)?;
    Ok(())
}

#[test]
fn one_shot_coalescing_is_complete_with_competing_registrations()
-> Result<(), Box<dyn std::error::Error>> {
    let (first, mut first_peer) = UnixStream::pair()?;
    let (second, mut second_peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(1, 2)?;
    let interest = Interest::READABLE | Interest::WRITABLE;
    let first_registration = poll.register(&first, Key::new(51), interest, Mode::OneShot)?;
    let second_registration = poll.register(&second, Key::new(52), interest, Mode::OneShot)?;
    second_peer.write_all(b"second readable")?;
    first_peer.write_all(b"first readable")?;

    let mut events = poll.events()?;
    let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    let [Event::Resource { key, readiness, .. }] = events.as_slice() else {
        return Err(io::Error::other("expected one coalesced resource event").into());
    };
    assert!(readiness.contains(zio::Readiness::READABLE.union(zio::Readiness::WRITABLE)));
    require_no_recovery(report)?;
    let armed = RegistrationState::Registered {
        arm: ArmState::Armed,
    };
    let disarmed = RegistrationState::Registered {
        arm: ArmState::Disarmed,
    };
    if *key == Key::new(51) {
        assert_eq!(poll.registration_state(&first_registration)?, disarmed);
        assert_eq!(poll.registration_state(&second_registration)?, armed);
    } else {
        assert_eq!(*key, Key::new(52));
        assert_eq!(poll.registration_state(&first_registration)?, armed);
        assert_eq!(poll.registration_state(&second_registration)?, disarmed);
    }

    poll.delete(first_registration)?;
    poll.delete(second_registration)?;
    Ok(())
}

#[test]
fn wait_rejects_an_undersized_destination() -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = Poll::with_capacity(4, 4)?;
    let mut events = Events::with_capacity(3)?;
    let result = poll.wait(&mut events, Wait::NoBlock);
    assert!(matches!(
        result,
        Err(Error::EventsTooSmall {
            required: 4,
            actual: 3
        })
    ));
    assert!(events.is_empty());
    Ok(())
}

#[test]
fn wait_rejection_clears_a_previous_observation() -> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    let mut source_poll = Poll::with_capacity(1, 1)?;
    let registration =
        source_poll.register(&source, Key::new(44), Interest::READABLE, Mode::Level)?;
    peer.write_all(b"ready")?;
    let mut events = source_poll.events()?;
    source_poll
        .wait(&mut events, Wait::For(Duration::from_secs(1)))?
        .into_result()?;
    assert!(!events.is_empty());

    let mut rejecting_poll = Poll::with_capacity(2, 1)?;
    assert!(matches!(
        rejecting_poll.wait(&mut events, Wait::NoBlock),
        Err(Error::EventsTooSmall { .. })
    ));
    assert!(events.is_empty());
    source_poll.delete(registration)?;
    Ok(())
}

#[test]
fn zero_event_capacity_is_reported_as_capacity_failure() {
    assert!(matches!(
        Events::with_capacity(0),
        Err(Error::Capacity {
            kind: CapacityKind::Event,
            limit: 0,
            reason: CapacityReason::Zero,
            ..
        })
    ));
}

#[test]
fn oversized_event_capacity_is_reported_without_panicking() {
    assert!(matches!(
        Events::with_capacity(usize::MAX),
        Err(Error::Capacity {
            kind: CapacityKind::Event,
            limit,
            reason: CapacityReason::StorageUnavailable,
            ..
        }) if limit == usize::MAX
    ));
}

fn has_key(events: &Events, expected: Key) -> bool {
    events
        .iter()
        .any(|event| matches!(event, Event::Resource { key, .. } if *key == expected))
}
