//! Unix-stream and TCP-stream readiness fixtures.

use std::{
    io::{Read, Write},
    net::{Ipv4Addr, Shutdown, TcpListener, TcpStream},
    os::unix::net::UnixStream,
};

use crate::readiness_expectation::expected_for;
use crate::readiness_pending::observe_pending_eof;
use crate::readiness_split::observe_split_eof;
use crate::readiness_verify::{mismatch, observe, observed};
use crate::{ReadinessCheck, ReadinessFailure, ReadinessScenario};

const PAYLOAD: &[u8] = b"zio-readiness";

pub(crate) fn unix_pending_eof(scenario: ReadinessScenario) -> Result<(), ReadinessFailure> {
    let (mut source, mut peer) = UnixStream::pair()
        .map_err(|error| observed(scenario, ReadinessCheck::Setup, "UnixStream pair", &error))?;
    source.set_nonblocking(true).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "nonblocking Unix source",
            &error,
        )
    })?;
    peer.write_all(PAYLOAD).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "buffered Unix payload",
            &error,
        )
    })?;
    peer.shutdown(Shutdown::Write).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "Unix write-half close",
            &error,
        )
    })?;

    observe_pending_eof(&mut source, PAYLOAD, scenario)
}

pub(crate) fn unix_writable(scenario: ReadinessScenario) -> Result<(), ReadinessFailure> {
    let (mut source, mut peer) = UnixStream::pair()
        .map_err(|error| observed(scenario, ReadinessCheck::Setup, "UnixStream pair", &error))?;
    source.set_nonblocking(true).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "nonblocking Unix source",
            &error,
        )
    })?;

    observe(&mut source, scenario, expected_for(scenario), |source| {
        source.write_all(b"w").map_err(|error| {
            observed(
                scenario,
                ReadinessCheck::Operation,
                "one successful write",
                &error,
            )
        })?;
        let mut byte = [0_u8; 1];
        peer.read_exact(&mut byte).map_err(|error| {
            observed(
                scenario,
                ReadinessCheck::Operation,
                "peer received one byte",
                &error,
            )
        })?;
        if byte == *b"w" {
            Ok(())
        } else {
            mismatch(scenario, ReadinessCheck::Operation, *b"w", byte)
        }
    })
}

pub(crate) fn tcp_pending_eof(scenario: ReadinessScenario) -> Result<(), ReadinessFailure> {
    let (mut source, mut peer) = tcp_pair(scenario)?;
    source.set_nonblocking(true).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "nonblocking TCP source",
            &error,
        )
    })?;
    peer.write_all(PAYLOAD).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "buffered TCP payload",
            &error,
        )
    })?;
    observe_split_eof(&mut source, PAYLOAD, scenario, || {
        peer.shutdown(Shutdown::Write).map_err(|error| {
            observed(
                scenario,
                ReadinessCheck::Setup,
                "TCP write-half close",
                &error,
            )
        })
    })
}

fn tcp_pair(scenario: ReadinessScenario) -> Result<(TcpStream, TcpStream), ReadinessFailure> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "loopback TCP listener",
            &error,
        )
    })?;
    let address = listener.local_addr().map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "loopback listener address",
            &error,
        )
    })?;
    let source = TcpStream::connect(address).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "connected loopback source",
            &error,
        )
    })?;
    let (peer, _) = listener.accept().map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "accepted loopback peer",
            &error,
        )
    })?;
    Ok((source, peer))
}
