//! Bounded readiness delivery fairness.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use std::{collections::HashSet, io::Write, os::unix::net::UnixStream, time::Duration};

use zio::{Interest, Key, Mode, Poll, Wait};

#[test]
fn ready_set_larger_than_event_capacity_makes_progress() -> Result<(), Box<dyn std::error::Error>> {
    verify(Mode::Level)?;
    verify(Mode::OneShot)
}

fn verify(mode: Mode) -> Result<(), Box<dyn std::error::Error>> {
    let mut poll = Poll::builder()
        .event_capacity(1)
        .registration_capacity(3)
        .build()?;
    let mut sources = Vec::new();
    let mut peers = Vec::new();
    let mut registrations = Vec::new();
    for value in 0..3 {
        let (source, mut peer) = UnixStream::pair()?;
        source.set_nonblocking(true)?;
        peer.write_all(b"ready")?;
        registrations.push(poll.register(&source, Key::new(value), Interest::READABLE, mode)?);
        sources.push(source);
        peers.push(peer);
    }

    let mut events = poll.events()?;
    let mut seen = HashSet::new();
    for attempt in 0..6 {
        let wait = if attempt == 0 {
            Wait::For(Duration::from_secs(1))
        } else {
            Wait::NoBlock
        };
        poll.wait(&mut events, wait)?.into_result()?;
        seen.extend(events.iter().map(|event| event.key()));
        if seen.len() == registrations.len() {
            break;
        }
    }

    assert_eq!(seen.len(), registrations.len());
    poll.delete_all()?;
    drop((sources, peers));
    Ok(())
}
