//! Black-box refused-connect readiness and `SO_ERROR` evidence.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

mod support;

use std::{fmt::Debug, io, net::TcpListener, time::Duration};

use socket2::{Domain, Protocol, Socket, Type};
use zio::{ArmState, Event, Interest, Key, Mode, Readiness, RegistrationState, Wait};

use support::require_no_recovery;

const KEY: Key = Key::new(6_001);
const DEADLINE: Duration = Duration::from_secs(1);

#[test]
fn refused_connect_level_reports_native_terminal_evidence() -> Result<(), Box<dyn std::error::Error>>
{
    verify_refused_connect(Mode::Level)
}

#[test]
fn refused_connect_one_shot_reports_native_terminal_evidence()
-> Result<(), Box<dyn std::error::Error>> {
    verify_refused_connect(Mode::OneShot)
}

fn verify_refused_connect(mode: Mode) -> Result<(), Box<dyn std::error::Error>> {
    let stream = match refused_stream()? {
        RefusedConnect::Pending(stream) => stream,
        RefusedConnect::Immediate(error) => {
            if error.kind() == io::ErrorKind::ConnectionRefused
                && error.raw_os_error() == Some(libc::ECONNREFUSED)
            {
                return Ok(());
            }
            return Err(failure("immediate ECONNREFUSED", error).into());
        }
    };
    let mut poll = zio::Poll::with_capacity(1, 1)?;
    let registration = poll.register(&stream, KEY, Interest::WRITABLE, mode)?;
    let mut events = poll.events()?;
    let report = poll.wait(&mut events, Wait::For(DEADLINE))?;

    let readiness = match events.as_slice() {
        [Event::Resource { key, readiness }] if *key == KEY => *readiness,
        actual => return Err(failure("one refused-connect resource event", actual).into()),
    };
    let required = Readiness::ERROR;
    let allowed = Readiness::ERROR
        .union(Readiness::WRITABLE)
        .union(Readiness::READ_CLOSED)
        .union(Readiness::WRITE_CLOSED);
    if !readiness.contains(required) {
        return Err(failure("ERROR readiness", readiness).into());
    }
    if !allowed.contains(readiness) {
        return Err(failure("only documented refused-connect hints", readiness).into());
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
        "registration state after refused connect",
    )?;
    if mode == Mode::OneShot {
        let report = poll.wait(&mut events, Wait::NoBlock)?;
        if !events.is_empty() {
            return Err(failure("no delivery before explicit rearm", events.as_slice()).into());
        }
        require_no_recovery(report)?;
    }

    match stream.take_error()? {
        Some(error) if error.raw_os_error().is_some_and(|code| code != 0) => {}
        actual => return Err(failure("nonzero refused-connect SO_ERROR", actual).into()),
    }
    poll.delete(registration)?;
    Ok(())
}

enum RefusedConnect {
    Pending(Socket),
    Immediate(io::Error),
}

fn refused_stream() -> io::Result<RefusedConnect> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let address = listener.local_addr()?;
    drop(listener);
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_nonblocking(true)?;
    match socket.connect(&address.into()) {
        Err(error) if connect_is_in_progress(&error) => Ok(RefusedConnect::Pending(socket)),
        Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
            Ok(RefusedConnect::Immediate(error))
        }
        Ok(()) => Err(io::Error::other(
            "refused-connect fixture unexpectedly connected",
        )),
        Err(error) => Err(error),
    }
}

fn connect_is_in_progress(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::WouldBlock || error.raw_os_error() == Some(libc::EINPROGRESS)
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
