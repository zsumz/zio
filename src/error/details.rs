//! Capability-preserving mutation and recovery failure details.

use std::{fmt, io};

use crate::{Registration, RegistrationId};

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

/// A wait-time recovery failure and every registration it affected.
#[derive(Debug)]
pub struct RecoveryFailure {
    operation: Operation,
    commit: CommitStatus,
    affected: Box<[RegistrationId]>,
    source: io::Error,
}

impl RecoveryFailure {
    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(crate) const fn new(
        operation: Operation,
        commit: CommitStatus,
        affected: Box<[RegistrationId]>,
        source: io::Error,
    ) -> Self {
        Self {
            operation,
            commit,
            affected,
            source,
        }
    }

    /// Returns the failed recovery operation.
    pub const fn operation(&self) -> Operation {
        self.operation
    }

    /// Returns the proven commit status.
    pub const fn commit(&self) -> CommitStatus {
        self.commit
    }

    /// Borrows exact registration identities affected by recovery.
    pub fn affected(&self) -> &[RegistrationId] {
        &self.affected
    }

    /// Returns the operating-system failure.
    pub const fn source(&self) -> &io::Error {
        &self.source
    }

    /// Splits this failure into its owned parts.
    pub fn into_parts(self) -> (Operation, CommitStatus, Box<[RegistrationId]>, io::Error) {
        (self.operation, self.commit, self.affected, self.source)
    }
}

impl fmt::Display for RecoveryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} recovery failed with {:?} commit status for {} registrations: {}",
            self.operation,
            self.commit,
            self.affected.len(),
            self.source
        )
    }
}

impl std::error::Error for RecoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Registration failure that may preserve an uncertain partial registration.
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

    /// Borrows an uncertain partial registration, when one exists.
    pub const fn registration(&self) -> Option<&Registration> {
        self.registration.as_ref()
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

/// Delete failure that returns the move-only registration capability.
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

    /// Borrows the registration returned after failed deletion.
    pub const fn registration(&self) -> &Registration {
        &self.registration
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
