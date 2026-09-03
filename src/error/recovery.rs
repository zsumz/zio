//! Exact owned outcomes for wait-time recovery failures.

use std::{fmt, io};

use crate::{
    Registration,
    registration::{ArmState, RegistrationState},
};

use super::{CommitStatus, Operation};

/// The exact result of recovering one attempted registration.
///
/// The commit status and authoritative state always agree: applied recovery
/// leaves a one-shot registration disarmed, proven-not-applied recovery leaves
/// it armed, and an unprovable recovery leaves it uncertain. This is an owned
/// historical snapshot with its actionable handle; later poller mutations do
/// not change it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecoveryOutcome {
    registration: Registration,
    commit: CommitStatus,
    state: RegistrationState,
}

impl RecoveryOutcome {
    #[allow(
        dead_code,
        reason = "only kqueue targets construct wait-time recovery outcomes"
    )]
    pub(crate) const fn new(registration: Registration, commit: CommitStatus) -> Self {
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

    /// Returns the exact attempted registration handle.
    pub const fn registration(&self) -> Registration {
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

    /// Returns the number of exact registration outcomes.
    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    /// Returns whether no registration outcome is retained.
    pub fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }

    /// Iterates over exact outcomes in observation order.
    pub fn iter(&self) -> core::slice::Iter<'_, RecoveryOutcome> {
        self.outcomes.iter()
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

impl AsRef<[RecoveryOutcome]> for RecoveryFailure {
    fn as_ref(&self) -> &[RecoveryOutcome] {
        self.outcomes()
    }
}

impl<'a> IntoIterator for &'a RecoveryFailure {
    type Item = &'a RecoveryOutcome;
    type IntoIter = core::slice::Iter<'a, RecoveryOutcome>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl fmt::Display for RecoveryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let registration = if self.len() == 1 {
            "registration"
        } else {
            "registrations"
        };
        write!(
            formatter,
            "{} recovery failed for {} {registration}: {}",
            self.operation,
            self.len(),
            self.source
        )
    }
}

impl std::error::Error for RecoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
