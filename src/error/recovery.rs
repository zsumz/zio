//! Exact owned outcomes for wait-time recovery failures.

use std::{fmt, io};

use crate::registration::{ArmState, RegistrationId, RegistrationState};

use super::{CommitStatus, Operation};

/// The exact result of recovering one attempted registration.
///
/// The commit status and authoritative state always agree: applied recovery
/// leaves a one-shot registration disarmed, proven-not-applied recovery leaves
/// it armed, and an unprovable recovery leaves it uncertain. This is an owned
/// historical snapshot for the exact generation; later poller mutations do not
/// change it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecoveryOutcome {
    registration: RegistrationId,
    commit: CommitStatus,
    state: RegistrationState,
}

impl RecoveryOutcome {
    #[allow(
        dead_code,
        reason = "only kqueue targets construct wait-time recovery outcomes"
    )]
    pub(crate) const fn new(registration: RegistrationId, commit: CommitStatus) -> Self {
        let state = match commit {
            CommitStatus::Applied => RegistrationState::Registered {
                arm: ArmState::Disarmed,
            },
            CommitStatus::NotApplied => RegistrationState::Registered {
                arm: ArmState::Armed,
            },
            CommitStatus::Unknown => RegistrationState::Uncertain,
        };
        Self {
            registration,
            commit,
            state,
        }
    }

    /// Returns the exact attempted registration generation.
    pub const fn registration(&self) -> RegistrationId {
        self.registration
    }

    /// Returns the proven commit status for this registration's disarm.
    pub const fn commit(&self) -> CommitStatus {
        self.commit
    }

    /// Returns the state established by this recovery attempt.
    pub const fn state(&self) -> RegistrationState {
        self.state
    }
}

/// A wait-time recovery failure with exact owned per-registration outcomes.
///
/// Outcomes include every one-shot registration attempted by the failed
/// recovery batch, in observation order. The source is the first native or
/// receipt-protocol failure in submitted-change order.
///
/// Successful recovery does not create this report allocation. Each failed
/// post-delivery recovery owns one `Vec` backing allocation containing at most
/// the smaller of the poller's event and registration capacities. Retaining
/// several reports retains one independent bounded allocation per failure and
/// does not borrow the poller. Allocation exhaustion follows Rust's ordinary
/// allocation-error policy.
#[derive(Debug)]
pub struct RecoveryFailure {
    operation: Operation,
    outcomes: Vec<RecoveryOutcome>,
    source: io::Error,
}

impl RecoveryFailure {
    #[cfg(any(test, target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(crate) const fn new(
        operation: Operation,
        outcomes: Vec<RecoveryOutcome>,
        source: io::Error,
    ) -> Self {
        Self {
            operation,
            outcomes,
            source,
        }
    }

    /// Returns the failed recovery operation.
    pub const fn operation(&self) -> Operation {
        self.operation
    }

    /// Borrows every exact registration outcome in observation order.
    pub fn outcomes(&self) -> &[RecoveryOutcome] {
        &self.outcomes
    }

    /// Returns the first native or receipt-protocol failure in submission order.
    pub const fn source(&self) -> &io::Error {
        &self.source
    }

    /// Splits this failure into its operation, exact ordered outcomes, and
    /// first native or receipt-protocol source.
    pub fn into_parts(self) -> (Operation, Vec<RecoveryOutcome>, io::Error) {
        (self.operation, self.outcomes, self.source)
    }
}

impl fmt::Display for RecoveryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} recovery failed for {} registrations: {}",
            self.operation,
            self.outcomes.len(),
            self.source
        )
    }
}

impl std::error::Error for RecoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
