//! Native selector descriptor composition.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

mod support;

use std::{
    io,
    os::fd::{AsFd, AsRawFd},
    time::Duration,
};

use zio::{Event, Interest, Key, Mode, Poll, Wait};

use support::descriptor_flags;

#[test]
fn raw_descriptor_matches_the_safe_borrow() -> Result<(), zio::Error> {
    let poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;

    assert_eq!(poll.as_raw_fd(), poll.as_fd().as_raw_fd());
    Ok(())
}

#[test]
fn selector_descriptor_is_close_on_exec() -> Result<(), Box<dyn std::error::Error>> {
    let poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;

    let flags = descriptor_flags(poll.as_fd())?;
    assert_ne!(flags & libc::FD_CLOEXEC, 0);
    Ok(())
}

#[test]
fn nested_poller_readiness_clears_and_reactivates() -> Result<(), Box<dyn std::error::Error>> {
    let mut inner = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let mut outer = Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let nested = outer.register(&inner, Key::new(1), Interest::READABLE, Mode::Level)?;
    let waker = inner.waker(Key::new(2))?;
    waker.wake()?;

    let mut outer_events = outer.events()?;
    outer
        .wait(&mut outer_events, Wait::For(Duration::from_secs(1)))?
        .into_result()?;
    let [
        Event::Resource {
            registration,
            key,
            readiness,
            ..
        },
    ] = outer_events.as_slice()
    else {
        return Err(io::Error::other("expected one nested poller event").into());
    };
    assert_eq!((*registration, *key), (nested, Key::new(1)));
    assert!(readiness.is_readable());

    let mut inner_events = inner.events()?;
    inner
        .wait(&mut inner_events, Wait::NoBlock)?
        .into_result()?;
    assert!(matches!(
        inner_events.as_slice(),
        [Event::Wake { key, .. }] if *key == Key::new(2)
    ));

    outer
        .wait(&mut outer_events, Wait::NoBlock)?
        .into_result()?;
    assert!(outer_events.is_empty());

    waker.wake()?;
    outer
        .wait(&mut outer_events, Wait::For(Duration::from_secs(1)))?
        .into_result()?;
    assert!(matches!(
        outer_events.as_slice(),
        [Event::Resource {
            registration,
            key,
            readiness,
            ..
        }] if *registration == nested && *key == Key::new(1) && readiness.is_readable()
    ));

    outer.delete(nested)?;
    Ok(())
}
