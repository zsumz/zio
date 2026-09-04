//! Black-box abortive TCP-close readiness evidence.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

mod support;

use std::{
    fmt::Debug,
    io::{self, Read},
    net::{Ipv4Addr, TcpListener, TcpStream},
    time::Duration,
};

use socket2::SockRef;
use zio::{ArmState, Event, Interest, Key, Mode, Readiness, RegistrationState, Wait};

use support::require_no_recovery;

const KEY: Key = Key::new(6_101);
const DEADLINE: Duration = Duration::from_secs(1);

#[test]
fn abortive_peer_close_reports_error_and_read_closure() -> Result<(), Box<dyn std::error::Error>> {
    verify_abortive_close(Mode::Level)
}

#[test]
fn abortive_peer_close_one_shot_disarms_exactly_once() -> Result<(), Box<dyn std::error::Error>> {
    verify_abortive_close(Mode::OneShot)
}

fn verify_abortive_close(mode: Mode) -> Result<(), Box<dyn std::error::Error>> {
    let (mut source, peer) = tcp_pair()?;
    source.set_nonblocking(true)?;

    let mut poll = zio::Poll::builder()
        .event_capacity(1)
        .registration_capacity(1)
        .build()?;
    let registration = poll.register(&source, KEY, Interest::READABLE, mode)?;
    let mut events = poll.events()?;

    SockRef::from(&peer).set_linger(Some(Duration::ZERO))?;
    drop(peer);
    let report = poll.wait(&mut events, Wait::For(DEADLINE))?;

    let readiness = match events.as_slice() {
        [Event::Resource { key, readiness, .. }] if *key == KEY => *readiness,
        actual => return Err(failure("one abortive-close resource event", actual).into()),
    };
    let required = Readiness::READ_CLOSED.union(Readiness::ERROR);
    let allowed = Readiness::READABLE
        .union(Readiness::READ_CLOSED)
        .union(Readiness::WRITE_CLOSED)
        .union(Readiness::ERROR);
    if !readiness.contains(required) {
        return Err(failure("READ_CLOSED and ERROR readiness", readiness).into());
    }
    if !allowed.contains(readiness) {
        return Err(failure("only documented abortive-close hints", readiness).into());
    }
    require_no_recovery(report)?;

    let arm = if mode == Mode::OneShot {
        ArmState::Disarmed
    } else {
        ArmState::Armed
    };
    check_eq(
        &poll.registration_state(&registration)?,
        &RegistrationState::Registered { arm },
        "registration state after abortive close",
    )?;
    if mode == Mode::OneShot {
        let report = poll.wait(&mut events, Wait::NoBlock)?;
        if !events.is_empty() {
            return Err(failure("no delivery before explicit rearm", events.as_slice()).into());
        }
        require_no_recovery(report)?;
    }

    let mut byte = [0_u8; 1];
    match source.read(&mut byte) {
        Err(error) if error.kind() != io::ErrorKind::WouldBlock => {}
        actual => return Err(failure("a concrete reset read error", actual).into()),
    }
    poll.delete(registration)?;
    Ok(())
}

fn tcp_pair() -> io::Result<(TcpStream, TcpStream)> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let source = TcpStream::connect(listener.local_addr()?)?;
    let (peer, _) = listener.accept()?;
    Ok((source, peer))
}

fn check_eq<T: Debug + Eq>(actual: &T, expected: &T, context: &str) -> io::Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(failure(context, actual))
    }
}

fn failure(expected: &str, actual: impl Debug) -> io::Error {
    io::Error::other(format!("expected {expected}, observed {actual:?}"))
}
