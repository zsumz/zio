//! Cancellation-safe registration-handle lifetime evidence.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use std::{
    hash::Hash,
    io::{self, Read},
    os::{fd::OwnedFd, unix::net::UnixStream},
    time::Duration,
};

use zio::{
    ArmState, DeleteError, DeleteOwnedError, Error, Interest, Key, Mode, Poll, Registration,
    RegistrationState,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn registration_handle_traits_are_stable() -> TestResult {
    fn assert_traits<T: Clone + Copy + Eq + Hash + Ord + Send + Sync>() {}

    assert_traits::<Registration>();
    ensure(
        !std::mem::needs_drop::<Registration>(),
        "registration unexpectedly requires drop",
    )
}

#[test]
fn reactor_copy_reclaims_after_task_copy_is_abandoned() -> TestResult {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let registered = poll.register(&source, Key::new(901), Interest::READABLE, Mode::Level)?;
    let reactor = registered;

    abandon_task_handle(registered);
    poll.delete(reactor)?;
    Ok(())
}

#[test]
fn dropping_poller_closes_a_transferred_descriptor() -> TestResult {
    let (source, mut peer) = UnixStream::pair()?;
    peer.set_read_timeout(Some(Duration::from_secs(1)))?;
    let descriptor: OwnedFd = source.into();
    let mut poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let _registration =
        poll.register_owned(descriptor, Key::new(907), Interest::READABLE, Mode::Level)?;

    drop(poll);

    ensure(
        peer.read(&mut [0_u8])? == 0,
        "transferred descriptor remained open",
    )
}

#[test]
fn live_waker_does_not_retain_a_transferred_descriptor() -> TestResult {
    let (source, mut peer) = UnixStream::pair()?;
    peer.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let _registration = poll.register_owned(
        source.into(),
        Key::new(908),
        Interest::READABLE,
        Mode::Level,
    )?;
    let waker = poll.waker(Key::new(909))?;

    drop(poll);

    ensure(
        peer.read(&mut [0_u8])? == 0,
        "live waker retained a transferred descriptor",
    )?;
    waker.wake()?;
    Ok(())
}

#[test]
fn successful_delete_stales_every_surviving_copy() -> TestResult {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let registered = poll.register(&source, Key::new(902), Interest::READABLE, Mode::Level)?;
    let first_survivor = registered;
    let second_survivor = registered;
    let retired = registered.id();

    poll.delete(registered)?;
    expect_stale(poll.registration_state(&first_survivor), retired)?;
    expect_stale(
        poll.modify(&second_survivor, Interest::WRITABLE, Mode::OneShot),
        retired,
    )?;
    let Err(error) = poll.delete(first_survivor) else {
        return Err(io::Error::other("a surviving copy deleted a retired generation").into());
    };
    let (cause, returned) = error.into_parts();
    ensure(
        returned == first_survivor,
        "stale delete returned another handle",
    )?;
    expect_stale_error(cause, retired)?;
    Ok(())
}

#[test]
fn owned_delete_returns_a_stale_handle_without_a_descriptor() -> TestResult {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let registration = poll.register(&source, Key::new(905), Interest::READABLE, Mode::Level)?;
    poll.delete(registration)?;

    let Err(DeleteOwnedError::Retained {
        error,
        registration: returned,
    }) = poll.delete_owned(registration)
    else {
        return Err(io::Error::other("stale owned deletion returned a descriptor").into());
    };

    ensure(
        returned == registration,
        "stale owned deletion changed the handle",
    )?;
    expect_stale_error(error, registration.id())
}

#[test]
fn owned_delete_returns_a_foreign_handle_without_mutation() -> TestResult {
    let (source, _peer) = UnixStream::pair()?;
    let mut owner = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let mut stranger = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let registration = owner.register(&source, Key::new(906), Interest::READABLE, Mode::Level)?;

    let Err(DeleteOwnedError::Retained {
        error,
        registration: returned,
    }) = stranger.delete_owned(registration)
    else {
        return Err(io::Error::other("foreign owned deletion returned a descriptor").into());
    };

    ensure(
        returned == registration,
        "foreign owned deletion changed the handle",
    )?;
    ensure(
        matches!(error, Error::WrongPoller { registration: rejected } if rejected == registration),
        "foreign owned deletion lost its cause",
    )?;
    ensure(
        owner.contains(&registration),
        "foreign deletion mutated the owner",
    )?;
    owner.delete(registration)?;
    Ok(())
}

#[test]
fn stale_copy_cannot_target_reused_generation() -> TestResult {
    let (first_source, _first_peer) = UnixStream::pair()?;
    let (replacement_source, _replacement_peer) = UnixStream::pair()?;
    let mut poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let first = poll.register(
        &first_source,
        Key::new(903),
        Interest::READABLE,
        Mode::Level,
    )?;
    let stale = first;
    let retired = first.id();
    poll.delete(first)?;

    let replacement = poll.register(
        &replacement_source,
        Key::new(904),
        Interest::WRITABLE,
        Mode::OneShot,
    )?;
    if replacement.id() == retired {
        return Err(io::Error::other("slot reuse repeated a retired generation").into());
    }
    expect_stale(
        poll.modify(&stale, Interest::READABLE, Mode::OneShot),
        retired,
    )?;
    let error = delete_error(poll.delete(stale))?;
    let (cause, returned) = error.into_parts();
    ensure(returned == stale, "stale delete returned another handle")?;
    expect_stale_error(cause, retired)?;
    check_eq(
        &poll.registration_state(&replacement)?,
        &RegistrationState::Registered {
            arm: ArmState::Armed,
        },
        "replacement state after stale-copy operations",
    )?;
    poll.delete(replacement)?;
    Ok(())
}

fn abandon_task_handle(_registration: Registration) {}

fn delete_error(result: Result<(), DeleteError>) -> Result<DeleteError, io::Error> {
    match result {
        Err(error) => Ok(error),
        Ok(()) => Err(io::Error::other("stale delete unexpectedly succeeded")),
    }
}

fn expect_stale<T>(result: Result<T, Error>, expected: zio::RegistrationId) -> TestResult {
    match result {
        Err(Error::Stale { registration }) if registration == expected => Ok(()),
        Err(actual) => Err(io::Error::other(format!(
            "expected stale registration {expected:?}, observed {actual}"
        ))
        .into()),
        Ok(_) => Err(io::Error::other("stale registration operation succeeded").into()),
    }
}

fn expect_stale_error(error: Error, expected: zio::RegistrationId) -> TestResult {
    match error {
        Error::Stale { registration } if registration == expected => Ok(()),
        actual => Err(io::Error::other(format!(
            "expected stale registration {expected:?}, observed {actual}"
        ))
        .into()),
    }
}

fn check_eq<T: std::fmt::Debug + Eq + ?Sized>(
    actual: &T,
    expected: &T,
    context: &str,
) -> TestResult {
    if actual == expected {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{context}: expected {expected:?}, observed {actual:?}"
        ))
        .into())
    }
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message).into())
    }
}
