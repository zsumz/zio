//! Structured error diagnostics.

use std::io;

use crate::{Key, RegistrationId};

use super::{CommitStatus, Error, Operation};

impl Error {
    /// Returns the associated operation, when one is recorded.
    pub const fn operation(&self) -> Option<Operation> {
        match self {
            Self::Io { operation, .. } => Some(*operation),
            Self::Mutation(error) => Some(error.operation()),
            Self::UnsupportedPlatform => Some(Operation::UnsupportedPlatform),
            _ => None,
        }
    }

    /// Returns the mutation commit status, when present.
    pub const fn commit(&self) -> Option<CommitStatus> {
        match self {
            Self::Mutation(error) => Some(error.commit()),
            _ => None,
        }
    }

    /// Returns the associated poller-local registration ID, when present.
    pub const fn registration_id(&self) -> Option<RegistrationId> {
        match self {
            Self::WrongPoller { registration } => Some(registration.id()),
            Self::Stale { registration } | Self::Uncertain { registration } => Some(*registration),
            _ => None,
        }
    }

    /// Returns `(existing, requested)` for a conflicting wake key.
    pub const fn waker_key_conflict(&self) -> Option<(Key, Key)> {
        match self {
            Self::WakerAlreadyConfigured {
                existing,
                requested,
            } => Some((*existing, *requested)),
            _ => None,
        }
    }

    /// Returns the fixed capacity associated with a capacity failure.
    pub const fn capacity_limit(&self) -> Option<usize> {
        match self {
            Self::Capacity { limit } => Some(*limit),
            _ => None,
        }
    }

    /// Returns `(required, actual)` for an undersized event destination.
    pub const fn event_capacity_mismatch(&self) -> Option<(usize, usize)> {
        match self {
            Self::EventsTooSmall { required, actual } => Some((*required, *actual)),
            _ => None,
        }
    }

    /// Returns the underlying I/O error, when present.
    pub const fn io_error(&self) -> Option<&io::Error> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Mutation(error) => Some(error.source()),
            _ => None,
        }
    }
}
