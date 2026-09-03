//! Portable setup, mutation, wait, wake, and recovery failures.

mod accessors;
mod capacity;
mod delete_owned;
mod details;
#[cfg(test)]
mod details_test;
mod recovery;
mod register_owned;

#[cfg(test)]
mod recovery_test;

use std::{fmt, io};

use crate::{Key, Registration, RegistrationId};

pub use capacity::{CapacityKind, CapacityReason};
pub use delete_owned::DeleteOwnedError;
pub use details::{DeleteAllError, DeleteError, MutationError, RegisterError};
pub use recovery::{RecoveryFailure, RecoveryOutcome};
pub use register_owned::RegisterOwnedError;

/// Poller or backend operation associated with a failure.
///
/// This diagnostic vocabulary may grow as Zio gains backend operations.
/// Downstream matches must include a fallback arm.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Operation {
    /// Create the platform selector.
    CreatePoller,
    /// Create the platform wake source.
    CreateWaker,
    /// Register the wake source with the selector.
    RegisterWaker,
    /// Register a resource.
    Register,
    /// Modify and rearm a resource.
    Modify,
    /// Delete a resource.
    Delete,
    /// Wait for backend observations.
    Wait,
    /// Trigger the configured wake source.
    TriggerWake,
    /// Disable a delivered one-shot registration.
    Disarm,
}

impl fmt::Display for Operation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CreatePoller => "create poller",
            Self::CreateWaker => "create waker",
            Self::RegisterWaker => "register waker",
            Self::Register => "register",
            Self::Modify => "modify",
            Self::Delete => "delete",
            Self::Wait => "wait",
            Self::TriggerWake => "trigger wake",
            Self::Disarm => "disarm",
        })
    }
}

/// What a failed synchronous mutation changed in the backend.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommitStatus {
    /// The requested mutation was not applied.
    NotApplied,
    /// The requested mutation's postcondition is proven.
    Applied,
    /// The resulting backend state cannot be proven.
    Unknown,
}

impl fmt::Display for CommitStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotApplied => "not applied",
            Self::Applied => "applied",
            Self::Unknown => "unknown",
        })
    }
}

/// Portable poller failure.
///
/// New diagnostic variants may be added without changing successful poller
/// behavior. Downstream matches must include a fallback arm.
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    /// A non-mutation backend I/O operation failed.
    Io {
        /// Failed operation.
        operation: Operation,
        /// Underlying operating-system failure.
        source: io::Error,
    },
    /// A backend mutation failed with an exact commit status.
    Mutation(MutationError),
    /// The poller wake source already carries another key.
    WakerAlreadyConfigured {
        /// Previously configured key.
        existing: Key,
        /// Rejected key.
        requested: Key,
    },
    /// The registration belongs to another poller.
    WrongPoller {
        /// Exact rejected registration handle.
        registration: Registration,
    },
    /// The requested readiness interest is empty or unsupported.
    InvalidInterest,
    /// The registration generation is no longer retained.
    Stale {
        /// Rejected registration.
        registration: RegistrationId,
    },
    /// The registration's backend state cannot be proven.
    Uncertain {
        /// Affected registration.
        registration: RegistrationId,
    },
    /// The registration retains a borrowed descriptor.
    DescriptorNotOwned {
        /// Rejected registration.
        registration: RegistrationId,
    },
    /// A requested fixed capacity was unavailable.
    #[non_exhaustive]
    Capacity {
        /// Storage category that was unavailable.
        kind: CapacityKind,
        /// Configured capacity.
        limit: usize,
        /// Why the capacity was unavailable.
        reason: CapacityReason,
    },
    /// Every registration generation in the fixed table is exhausted.
    RegistrationSpaceExhausted,
    /// The supplied event destination cannot hold a complete batch.
    EventsTooSmall {
        /// Required logical capacity.
        required: usize,
        /// Supplied logical capacity.
        actual: usize,
    },
    /// A validated internal invariant diverged.
    Invariant,
    /// The current target has no supported readiness backend.
    UnsupportedPlatform,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { operation, source } => write!(formatter, "{operation} failed: {source}"),
            Self::Mutation(error) => error.fmt(formatter),
            Self::WakerAlreadyConfigured {
                existing,
                requested,
            } => write!(
                formatter,
                "poller wake key is {existing}, not requested key {requested}"
            ),
            Self::WrongPoller { registration } => {
                write!(
                    formatter,
                    "registration {} belongs to another poller",
                    registration.id()
                )
            }
            Self::InvalidInterest => formatter.write_str("readiness interest must not be empty"),
            Self::Stale { registration } => {
                write!(formatter, "registration {registration} is stale")
            }
            Self::Uncertain { registration } => write!(
                formatter,
                "registration {registration} has uncertain backend state"
            ),
            Self::DescriptorNotOwned { registration } => {
                write!(
                    formatter,
                    "registration {registration} does not own its descriptor"
                )
            }
            Self::Capacity {
                kind,
                limit,
                reason,
            } => {
                write!(formatter, "{kind} capacity {limit} {reason}")
            }
            Self::RegistrationSpaceExhausted => {
                formatter.write_str("registration generation space is exhausted")
            }
            Self::EventsTooSmall { required, actual } => write!(
                formatter,
                "event capacity {actual} is smaller than required capacity {required}"
            ),
            Self::Invariant => formatter.write_str("validated internal invariant diverged"),
            Self::UnsupportedPlatform => {
                formatter.write_str("the current target has no supported readiness backend")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Mutation(error) => Some(error),
            _ => None,
        }
    }
}
