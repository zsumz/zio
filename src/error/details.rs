//! Capability-preserving mutation failure details.

use std::{fmt, io};

use crate::Registration;

use super::{CommitStatus, Error, Operation};

/// An I/O mutation failure with an exact commit status.
#[derive(Debug)]
pub struct MutationError {
    operation: Operation,
    commit: CommitStatus,
    source: io::Error,
}

impl MutationError {
    pub(crate) const fn new(operation: Operation, commit: CommitStatus, source: io::Error) -> Self {
        Self {
            operation,
            commit,
            source,
        }
    }

    /// Returns the failed operation.
    pub const fn operation(&self) -> Operation {
        self.operation
    }

    /// Returns the proven commit status.
    pub const fn commit(&self) -> CommitStatus {
        self.commit
    }

    /// Returns the operating-system failure.
    pub const fn source(&self) -> &io::Error {
        &self.source
    }

    /// Returns the owned operating-system failure.
    pub fn into_source(self) -> io::Error {
        self.source
    }
}

impl fmt::Display for MutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} failed with {:?} commit status: {}",
            self.operation, self.commit, self.source
        )
    }
}

impl std::error::Error for MutationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Registration failure that preserves any possibly installed registration.
///
/// [`Self::registration`] is `None` for preflight failures and mutations proven
/// not applied. It returns a registered handle for an applied mutation and an
/// uncertain handle when the backend result cannot be proven.
#[derive(Debug)]
pub struct RegisterError {
    error: Error,
    registration: Option<Registration>,
}

impl RegisterError {
    pub(crate) const fn new(error: Error, registration: Option<Registration>) -> Self {
        Self {
            error,
            registration,
        }
    }

    /// Returns the underlying failure.
    pub const fn error(&self) -> &Error {
        &self.error
    }

    /// Returns the retained applied or uncertain registration, when one exists.
    pub const fn registration(&self) -> Option<Registration> {
        self.registration
    }

    /// Splits this failure into the cause and optional registration.
    pub fn into_parts(self) -> (Error, Option<Registration>) {
        (self.error, self.registration)
    }
}

impl fmt::Display for RegisterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for RegisterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Delete failure that retains the exact registration handle.
///
/// Every handle copy remains registered after a not-applied failure, is stale
/// after an applied failure, and is uncertain after an unknown failure.
#[derive(Debug)]
pub struct DeleteError {
    error: Error,
    registration: Registration,
}

impl DeleteError {
    pub(crate) const fn new(error: Error, registration: Registration) -> Self {
        Self {
            error,
            registration,
        }
    }

    /// Returns the underlying failure.
    pub const fn error(&self) -> &Error {
        &self.error
    }

    /// Returns the registration retained after failed deletion.
    pub const fn registration(&self) -> Registration {
        self.registration
    }

    /// Splits this failure into the cause and returned registration.
    pub fn into_parts(self) -> (Error, Registration) {
        (self.error, self.registration)
    }
}

impl fmt::Display for DeleteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for DeleteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
