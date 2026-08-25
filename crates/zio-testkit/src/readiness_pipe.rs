//! Anonymous-pipe readiness fixtures.

use std::io::Write;

use crate::readiness_expectation::expected_for;
use crate::readiness_pending::observe_pending_eof;
use crate::readiness_verify::{mismatch, observe, observed};
use crate::{ReadinessCheck, ReadinessFailure, ReadinessScenario};

const PAYLOAD: &[u8] = b"zio-pipe";

pub(crate) fn pending_eof(scenario: ReadinessScenario) -> Result<(), ReadinessFailure> {
    let (mut source, mut peer) = std::io::pipe()
        .map_err(|error| observed(scenario, ReadinessCheck::Setup, "anonymous pipe", &error))?;
    peer.write_all(PAYLOAD).map_err(|error| {
        observed(
            scenario,
            ReadinessCheck::Setup,
            "buffered pipe payload",
            &error,
        )
    })?;
    drop(peer);

    observe_pending_eof(&mut source, PAYLOAD, scenario)
}

pub(crate) fn reader_closed(scenario: ReadinessScenario) -> Result<(), ReadinessFailure> {
    let (peer, mut source) = std::io::pipe()
        .map_err(|error| observed(scenario, ReadinessCheck::Setup, "anonymous pipe", &error))?;
    drop(peer);

    observe(
        &mut source,
        scenario,
        expected_for(scenario),
        |source| match source.write(b"closed") {
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
            actual => mismatch(
                scenario,
                ReadinessCheck::Operation,
                "BrokenPipe from write after reader close",
                actual,
            ),
        },
    )
}
