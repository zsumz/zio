//! Cross-thread wake lifecycle behavior.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

mod support;

use std::{io, thread, time::Duration};

use zio::{Event, Key, Poll, Wait};

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
    let mut poll = Poll::with_capacity(1, 1)?;
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
