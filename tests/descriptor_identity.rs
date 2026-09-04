//! Native descriptor identity and independent-registration behavior.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

mod support;

use std::{
    error::Error,
    io::{self, Write},
    os::{
        fd::{AsRawFd, RawFd},
        unix::net::UnixStream,
    },
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use zio::{
    ArmState, Event, Events, Interest, Key, Mode, Poll, Registration, RegistrationState, Wait,
};

use support::require_no_recovery;

const ORIGINAL_KEY: Key = Key::new(801);
const REUSED_KEY: Key = Key::new(802);
const FIRST_DUP_KEY: Key = Key::new(803);
const SECOND_DUP_KEY: Key = Key::new(804);
const FIRST_SHARED_KEY: Key = Key::new(805);
const SECOND_SHARED_KEY: Key = Key::new(806);
const OBSERVATION_LIMIT: Duration = Duration::from_secs(1);
const REUSE_ATTEMPTS: usize = 4_096;
static DESCRIPTOR_TESTS: Mutex<()> = Mutex::new(());
type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn numeric_descriptor_reuse_does_not_alias_retained_registration() -> TestResult {
    let _guard = descriptor_test_guard()?;
    let (source, mut original_peer) = UnixStream::pair()?;
    let caller_descriptor = source.as_raw_fd();
    let mut poll = Poll::builder()
        .event_capacity(2)
        .registration_capacity(2)
        .build()?;
    let original = poll.register(&source, ORIGINAL_KEY, Interest::READABLE, Mode::OneShot)?;
    drop(source);

    let (replacement, mut replacement_peer) = stream_with_descriptor(caller_descriptor)?;
    ensure(
        replacement.as_raw_fd() == caller_descriptor,
        "replacement did not reuse the released caller descriptor",
    )?;
    let reused = poll.register(&replacement, REUSED_KEY, Interest::READABLE, Mode::OneShot)?;
    ensure(
        original.id() != reused.id(),
        "independent resources received the same registration identity",
    )?;

    let mut events = poll.events()?;
    original_peer.write_all(b"original")?;
    let report = poll.wait(&mut events, Wait::For(OBSERVATION_LIMIT))?;
    expect_resources(&events, &[ORIGINAL_KEY])?;
    require_no_recovery(report)?;
    expect_arm(&poll, &original, ArmState::Disarmed)?;
    expect_arm(&poll, &reused, ArmState::Armed)?;

    replacement_peer.write_all(b"replacement")?;
    let report = poll.wait(&mut events, Wait::For(OBSERVATION_LIMIT))?;
    expect_resources(&events, &[REUSED_KEY])?;
    require_no_recovery(report)?;
    expect_arm(&poll, &original, ArmState::Disarmed)?;
    expect_arm(&poll, &reused, ArmState::Disarmed)?;

    poll.delete(original)?;
    poll.delete(reused)?;
    Ok(())
}

#[test]
fn duplicated_handles_have_independent_registrations() -> TestResult {
    let _guard = descriptor_test_guard()?;
    let (source, mut peer) = UnixStream::pair()?;
    let duplicate = source.try_clone()?;
    ensure(
        source.as_raw_fd() != duplicate.as_raw_fd(),
        "duplicated handles unexpectedly shared one numeric descriptor",
    )?;
    let mut poll = Poll::builder()
        .event_capacity(2)
        .registration_capacity(2)
        .build()?;
    let first = poll.register(&source, FIRST_DUP_KEY, Interest::READABLE, Mode::OneShot)?;
    let second = poll.register(
        &duplicate,
        SECOND_DUP_KEY,
        Interest::READABLE,
        Mode::OneShot,
    )?;
    ensure(
        first.id() != second.id(),
        "duplicated handles received the same registration identity",
    )?;

    peer.write_all(b"shared readiness")?;
    let mut events = poll.events()?;
    let report = poll.wait(&mut events, Wait::For(OBSERVATION_LIMIT))?;
    expect_resources(&events, &[FIRST_DUP_KEY, SECOND_DUP_KEY])?;
    require_no_recovery(report)?;
    expect_arm(&poll, &first, ArmState::Disarmed)?;
    expect_arm(&poll, &second, ArmState::Disarmed)?;

    poll.modify(&first, Interest::READABLE, Mode::OneShot)?;
    let report = poll.wait(&mut events, Wait::NoBlock)?;
    expect_resources(&events, &[FIRST_DUP_KEY])?;
    require_no_recovery(report)?;
    expect_arm(&poll, &first, ArmState::Disarmed)?;
    expect_arm(&poll, &second, ArmState::Disarmed)?;
    poll.delete(first)?;

    poll.modify(&second, Interest::READABLE, Mode::OneShot)?;
    let report = poll.wait(&mut events, Wait::NoBlock)?;
    expect_resources(&events, &[SECOND_DUP_KEY])?;
    require_no_recovery(report)?;
    poll.delete(second)?;
    Ok(())
}

#[test]
fn one_handle_can_have_independent_registrations() -> TestResult {
    let _guard = descriptor_test_guard()?;
    let (source, mut peer) = UnixStream::pair()?;
    let mut poll = Poll::builder()
        .event_capacity(2)
        .registration_capacity(2)
        .build()?;
    let first = poll.register(&source, FIRST_SHARED_KEY, Interest::READABLE, Mode::Level)?;
    let second = poll.register(&source, SECOND_SHARED_KEY, Interest::READABLE, Mode::Level)?;
    ensure(
        first.id() != second.id(),
        "repeated registration reused one registration identity",
    )?;

    peer.write_all(b"shared handle")?;
    let mut events = poll.events()?;
    let report = poll.wait(&mut events, Wait::For(OBSERVATION_LIMIT))?;
    expect_resources(&events, &[FIRST_SHARED_KEY, SECOND_SHARED_KEY])?;
    require_no_recovery(report)?;

    poll.delete(first)?;
    poll.delete(second)?;
    Ok(())
}

fn descriptor_test_guard() -> io::Result<MutexGuard<'static, ()>> {
    DESCRIPTOR_TESTS
        .lock()
        .map_err(|_| io::Error::other("descriptor test lock was poisoned"))
}

fn stream_with_descriptor(target: RawFd) -> io::Result<(UnixStream, UnixStream)> {
    // Retain lower-number candidates so allocation advances to the target.
    let mut fillers = Vec::new();
    for _attempt in 0..REUSE_ATTEMPTS {
        let (first, second) = UnixStream::pair()?;
        if first.as_raw_fd() == target {
            return Ok((first, second));
        }
        if second.as_raw_fd() == target {
            return Ok((second, first));
        }
        fillers.push((first, second));
    }
    Err(io::Error::other(format!(
        "could not force reuse of caller descriptor {target}"
    )))
}

fn expect_resources(events: &Events, expected: &[Key]) -> io::Result<()> {
    if events.len() != expected.len() {
        return Err(io::Error::other(format!(
            "expected resource keys {expected:?}, observed {:?}",
            events.as_slice()
        )));
    }
    for key in expected {
        let found = events.iter().any(|event| {
            matches!(
                event,
                Event::Resource {
                    key: actual,
                    readiness,
                    ..
                } if actual == key && readiness.is_readable()
            )
        });
        if !found {
            return Err(io::Error::other(format!(
                "missing readable resource key {key:?} in {:?}",
                events.as_slice()
            )));
        }
    }
    Ok(())
}

fn expect_arm(poll: &Poll, registration: &Registration, expected: ArmState) -> io::Result<()> {
    let actual = poll
        .registration_state(registration)
        .map_err(|error| io::Error::other(error.to_string()))?;
    let expected = RegistrationState::Registered { arm: expected };
    if actual == expected {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "expected {expected:?}, observed {actual:?}"
        )))
    }
}

fn ensure(condition: bool, message: &str) -> io::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message))
    }
}
