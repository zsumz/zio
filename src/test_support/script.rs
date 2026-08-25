//! Public normalized script and observation vocabulary.

use std::{fmt, io};

use crate::{
    ArmState, CommitStatus, Interest, Key, Mode, Operation, RegistrationId, RegistrationState,
};

/// Planned result of one scripted backend mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationOutcome {
    /// The backend mutation succeeds.
    Success,
    /// The backend mutation fails with an exact commit classification.
    Failure {
        /// What the failed mutation changed in the backend model.
        commit: CommitStatus,
        /// Error kind surfaced through the ordinary zio error.
        kind: io::ErrorKind,
    },
}

/// One operation-specific scripted backend result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationStep {
    /// Planned registration result.
    Register(MutationOutcome),
    /// Planned modification result.
    Modify(MutationOutcome),
    /// Planned deletion result.
    Delete(MutationOutcome),
}

impl MutationStep {
    pub(super) const fn operation(self) -> Operation {
        match self {
            Self::Register(_) => Operation::Register,
            Self::Modify(_) => Operation::Modify,
            Self::Delete(_) => Operation::Delete,
        }
    }

    pub(super) const fn outcome(self) -> MutationOutcome {
        match self {
            Self::Register(outcome) | Self::Modify(outcome) | Self::Delete(outcome) => outcome,
        }
    }
}

/// One normalized call observed by the scripted backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationCall {
    /// A registration request.
    Register {
        /// Exact registration generation.
        registration: RegistrationId,
        /// Caller-selected logical source marker.
        key: Key,
        /// Requested readiness interest.
        interest: Interest,
        /// Requested readiness mode.
        mode: Mode,
    },
    /// A modification request containing exact prior and desired state.
    Modify {
        /// Exact registration generation.
        registration: RegistrationId,
        /// Previously installed interest.
        previous_interest: Interest,
        /// Previously installed mode.
        previous_mode: Mode,
        /// Previous delivery eligibility.
        previous_arm: ArmState,
        /// Desired interest.
        desired_interest: Interest,
        /// Desired mode.
        desired_mode: Mode,
    },
    /// A deletion request.
    Delete {
        /// Exact registration generation.
        registration: RegistrationId,
        /// Last authoritative interest.
        interest: Interest,
        /// Last authoritative registration state.
        state: RegistrationState,
    },
    /// A test-only delivered one-shot observation.
    EstablishDisarmed {
        /// Exact registration generation.
        registration: RegistrationId,
    },
}

/// Normalized native state retained by the scripted backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptedBackendState {
    /// No backend registration is installed.
    Absent,
    /// The desired registration is installed exactly.
    Registered {
        /// Installed interest.
        interest: Interest,
        /// Installed mode.
        mode: Mode,
        /// Native delivery eligibility.
        arm: ArmState,
    },
    /// The installed backend state cannot be proven.
    Unknown,
}

/// Structural failure in a scripted mutation scenario.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptError {
    /// An operation had no remaining planned step.
    Exhausted {
        /// Unexpected operation.
        operation: Operation,
    },
    /// The next planned operation did not match the actual call.
    Mismatch {
        /// Planned operation.
        expected: Operation,
        /// Actual operation.
        actual: Operation,
    },
    /// Planned steps remained after the scenario ended.
    Remaining {
        /// Number of unconsumed steps.
        count: usize,
    },
    /// A retained registration changed its owned descriptor.
    DescriptorChanged {
        /// Affected registration.
        registration: RegistrationId,
    },
    /// A call referenced no modeled backend registration.
    UnknownRegistration {
        /// Missing registration.
        registration: RegistrationId,
    },
    /// A test tried to disarm a non-one-shot or non-installed registration.
    CannotDisarm {
        /// Rejected registration.
        registration: RegistrationId,
    },
}

impl fmt::Display for ScriptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "scripted mutation failure: {self:?}")
    }
}

impl std::error::Error for ScriptError {}
