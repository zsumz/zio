//! Structured error diagnostics.

use std::io;

use crate::{Key, Registration, RegistrationId};

use super::{CapacityKind, CapacityReason, CommitStatus, Error, Operation};

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
            Self::Stale { registration }
            | Self::Uncertain { registration }
            | Self::DescriptorNotOwned { registration } => Some(*registration),
            _ => None,
        }
    }

    /// Returns the exact rejected handle for a wrong-poller failure.
    pub const fn registration(&self) -> Option<Registration> {
        match self {
            Self::WrongPoller { registration } => Some(*registration),
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
            Self::Capacity { limit, .. } => Some(*limit),
            _ => None,
        }
    }

    /// Returns the storage category associated with a capacity failure.
    pub const fn capacity_kind(&self) -> Option<CapacityKind> {
        match self {
            Self::Capacity { kind, .. } => Some(*kind),
            _ => None,
        }
    }

    /// Returns why a fixed capacity was unavailable.
    pub const fn capacity_reason(&self) -> Option<CapacityReason> {
        match self {
            Self::Capacity { reason, .. } => Some(*reason),
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
