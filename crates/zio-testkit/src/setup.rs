//! Shared scripted scenario construction.

use std::{io, os::unix::net::UnixStream};

use zio::{
    ArmState, CommitStatus, Interest, Mode, RegistrationState,
    test_support::{MutationOutcome, ScriptedBackendState},
};

use crate::{Branch, ConformanceCheck, ConformanceFailure, MutationOperation, MutationScenario};

pub(crate) const PRIOR_INTEREST: Interest = Interest::READABLE;
pub(crate) const PRIOR_MODE: Mode = Mode::OneShot;
pub(crate) const DESIRED_INTEREST: Interest = Interest::WRITABLE;
pub(crate) const DESIRED_MODE: Mode = Mode::Level;

pub(crate) fn source(scenario: MutationScenario) -> Result<UnixStream, ConformanceFailure> {
    UnixStream::pair().map(|pair| pair.0).map_err(|error| {
        ConformanceFailure::new(
            scenario,
            ConformanceCheck::Setup,
            "Unix stream pair",
            error.to_string(),
        )
    })
}

pub(crate) fn outcome(scenario: MutationScenario) -> MutationOutcome {
    match scenario.branch() {
        Branch::Success => MutationOutcome::Success,
        branch => MutationOutcome::Failure {
            commit: match branch.commit() {
                Some(commit) => commit,
                None => CommitStatus::Unknown,
            },
            kind: source_kind(scenario.operation()),
        },
    }
}

pub(crate) const fn source_kind(operation: MutationOperation) -> io::ErrorKind {
    match operation {
        MutationOperation::Register => io::ErrorKind::PermissionDenied,
        MutationOperation::Modify => io::ErrorKind::TimedOut,
        MutationOperation::Delete => io::ErrorKind::BrokenPipe,
    }
}

pub(crate) const fn registered(arm: ArmState) -> RegistrationState {
    RegistrationState::Registered { arm }
}

pub(crate) const fn backend_registered(
    interest: Interest,
    mode: Mode,
    arm: ArmState,
) -> ScriptedBackendState {
    ScriptedBackendState::Registered {
        interest,
        mode,
        arm,
    }
}
