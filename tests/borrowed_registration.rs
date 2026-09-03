//! Black-box borrowed-registration ownership and identity behavior.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]
#![allow(
    unsafe_code,
    reason = "tests exercise the explicit borrowed-registration safety contract"
)]

mod support;

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

use zio::{ArmState, Error, Event, Interest, Key, Mode, Poll, RegistrationState, Wait};

use support::require_no_recovery;

const WAIT: Wait = Wait::For(Duration::from_secs(1));
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn borrowed_registration_supports_the_full_lifecycle() -> TestResult {
    let (mut source, mut peer) = UnixStream::pair()?;
    source.set_nonblocking(true)?;
    let mut poll = Poll::with_capacity(1, 1)?;
    // SAFETY: `source` remains open and unchanged through successful deletion.
    let registration =
        unsafe { poll.register_borrowed(&source, Key::new(1), Interest::READABLE, Mode::Level)? };
    let mut events = poll.events()?;

    peer.write_all(b"a")?;
    let report = poll.wait(&mut events, WAIT)?;
    expect_readable(&events, &[Key::new(1)])?;
    require_no_recovery(report)?;
    drain_byte(&mut source)?;

    poll.modify(&registration, Interest::READABLE, Mode::OneShot)?;
    peer.write_all(b"b")?;
    let report = poll.wait(&mut events, WAIT)?;
    expect_readable(&events, &[Key::new(1)])?;
    require_no_recovery(report)?;
    assert_eq!(
        poll.registration_state(&registration)?,
        RegistrationState::Registered {
            arm: ArmState::Disarmed,
        }
    );
    drain_byte(&mut source)?;
    poll.delete(registration)?;

    peer.write_all(b"c")?;
    drain_byte(&mut source)?;
    Ok(())
}

#[test]
fn dropping_poller_does_not_close_borrowed_source() -> TestResult {
    let (mut source, mut peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(1, 1)?;
    // SAFETY: `source` remains open and unchanged until `poll` is dropped.
    let _registration =
        unsafe { poll.register_borrowed(&source, Key::new(2), Interest::READABLE, Mode::Level)? };

    drop(poll);
    peer.write_all(b"a")?;
    drain_byte(&mut source)?;
    Ok(())
}

#[test]
fn borrowed_and_owned_slot_reuse_preserves_generations() -> TestResult {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(1, 1)?;
    // SAFETY: `source` remains open and unchanged through successful deletion.
    let borrowed =
        unsafe { poll.register_borrowed(&source, Key::new(3), Interest::READABLE, Mode::Level)? };
    let stale_borrowed = borrowed;
    poll.delete(borrowed)?;

    let owned = poll.register(&source, Key::new(4), Interest::READABLE, Mode::Level)?;
    assert_ne!(stale_borrowed.id(), owned.id());
    assert!(matches!(
        poll.modify(&stale_borrowed, Interest::WRITABLE, Mode::OneShot),
        Err(Error::Stale { registration }) if registration == stale_borrowed.id()
    ));
    let stale_owned = owned;
    poll.delete(owned)?;

    // SAFETY: `source` remains open and unchanged through successful deletion.
    let replacement =
        unsafe { poll.register_borrowed(&source, Key::new(5), Interest::READABLE, Mode::Level)? };
    assert_ne!(stale_owned.id(), replacement.id());
    assert!(matches!(
        poll.registration_state(&stale_owned),
        Err(Error::Stale { registration }) if registration == stale_owned.id()
    ));
    poll.delete(replacement)?;
    Ok(())
}

#[test]
fn duplicated_descriptors_have_independent_borrowed_registrations() -> TestResult {
    let (source, mut peer) = UnixStream::pair()?;
    source.set_nonblocking(true)?;
    let duplicate = source.try_clone()?;
    let mut poll = Poll::with_capacity(2, 2)?;
    // SAFETY: both distinct descriptors remain open through successful deletion.
    let first =
        unsafe { poll.register_borrowed(&source, Key::new(6), Interest::READABLE, Mode::OneShot)? };
    // SAFETY: `duplicate` has a distinct numeric descriptor and remains live.
    let second = unsafe {
        poll.register_borrowed(&duplicate, Key::new(7), Interest::READABLE, Mode::OneShot)?
    };

    peer.write_all(b"a")?;
    let mut events = poll.events()?;
    let report = poll.wait(&mut events, WAIT)?;
    expect_readable(&events, &[Key::new(6), Key::new(7)])?;
    require_no_recovery(report)?;
    poll.delete(first)?;
    poll.delete(second)?;
    Ok(())
}

fn drain_byte(source: &mut UnixStream) -> Result<(), std::io::Error> {
    let mut byte = [0_u8; 1];
    source.read_exact(&mut byte)
}

fn expect_readable(events: &zio::Events, expected: &[Key]) -> Result<(), std::io::Error> {
    if events.len() != expected.len() {
        return Err(std::io::Error::other(format!(
            "expected {expected:?}, observed {:?}",
            events.as_slice()
        )));
    }
    for expected_key in expected {
        let found = events.iter().any(|event| {
            matches!(
                event,
                Event::Resource { key, readiness, .. }
                    if key == expected_key && readiness.is_readable()
            )
        });
        if !found {
            return Err(std::io::Error::other(format!(
                "missing readable key {expected_key:?}"
            )));
        }
    }
    Ok(())
}
