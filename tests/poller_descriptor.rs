//! Native selector descriptor composition.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use std::{io, time::Duration};

use zio::{Event, Interest, Key, Mode, Poll, Wait};

#[test]
fn poller_can_be_observed_by_another_poller() -> Result<(), Box<dyn std::error::Error>> {
    let mut inner = Poll::with_capacity(1, 1)?;
    let mut outer = Poll::with_capacity(1, 1)?;
    let nested = outer.register(&inner, Key::new(1), Interest::READABLE, Mode::Level)?;
    inner.waker(Key::new(2))?.wake()?;

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

    outer.delete(nested)?;
    Ok(())
}
