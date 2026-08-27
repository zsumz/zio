//! Black-box allocation contract for native waiting.

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
    time::Duration,
};

use zio::{ArmState, Event, Interest, Key, Mode, Poll, RegistrationState, Wait};

use support::require_no_recovery;

#[test]
fn successful_one_shot_wait_allocates_nothing() -> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(1, 1)?;
    let registration = poll.register(&source, Key::new(81), Interest::READABLE, Mode::OneShot)?;
    let mut events = poll.events()?;
    peer.write_all(b"ready")?;
    let mut result = None;

    let allocations = allocation_counter::measure(|| {
        result = Some(poll.wait(&mut events, Wait::For(Duration::from_secs(1))));
    });

    let report = match result {
        Some(result) => result?,
        None => return Err(io::Error::other("measured wait did not complete").into()),
    };
    assert_eq!(allocations.count_total, 0);
    assert_eq!(allocations.count_current, 0);
    assert_eq!(allocations.count_max, 0);
    assert_eq!(allocations.bytes_total, 0);
    assert_eq!(allocations.bytes_current, 0);
    assert_eq!(allocations.bytes_max, 0);
    assert!(matches!(
        events.as_slice(),
        [Event::Resource { key, .. }] if *key == Key::new(81)
    ));
    require_no_recovery(report)?;
    assert_eq!(
        poll.registration_state(&registration)?,
        RegistrationState::Registered {
            arm: ArmState::Disarmed,
        }
    );
    poll.delete(registration)?;
    Ok(())
}

#[test]
fn successful_wake_roundtrip_allocates_nothing() -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = Poll::with_capacity(1, 1)?;
    let waker = poll.waker(Key::new(82))?;
    let mut events = poll.events()?;
    let mut wake_result = None;
    let mut wait_result = None;

    let allocations = allocation_counter::measure(|| {
        wake_result = Some(waker.wake());
        wait_result = Some(poll.wait(&mut events, Wait::For(Duration::from_secs(1))));
    });

    wake_result.ok_or_else(|| io::Error::other("measured wake did not complete"))??;
    let report =
        wait_result.ok_or_else(|| io::Error::other("measured wake wait did not complete"))??;
    assert_eq!(allocations.count_total, 0);
    assert_eq!(allocations.count_current, 0);
    assert_eq!(allocations.count_max, 0);
    assert_eq!(allocations.bytes_total, 0);
    assert_eq!(allocations.bytes_current, 0);
    assert_eq!(allocations.bytes_max, 0);
    assert!(matches!(
        events.as_slice(),
        [Event::Wake { key }] if *key == Key::new(82)
    ));
    require_no_recovery(report)?;
    Ok(())
}
