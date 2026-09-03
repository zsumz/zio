//! Owned-deletion capability failures.

use std::{fmt, os::fd::OwnedFd};

use crate::Registration;

use super::Error;

/// Owned deletion failure returning either the descriptor or attempted handle.
#[derive(Debug)]
pub enum DeleteOwnedError {
    /// The poller released the descriptor.
    Returned {
        /// Underlying failure.
        error: Error,
        /// Released descriptor.
        descriptor: OwnedFd,
    },
    /// No owned descriptor was released to the caller.
    Retained {
        /// Underlying failure.
        error: Error,
        /// Exact registration passed to deletion.
        registration: Registration,
    },
}

impl DeleteOwnedError {
    pub(crate) const fn returned(error: Error, descriptor: OwnedFd) -> Self {
        Self::Returned { error, descriptor }
    }

    pub(crate) const fn retained(error: Error, registration: Registration) -> Self {
        Self::Retained {
            error,
            registration,
        }
    }

    /// Returns the underlying failure.
    pub const fn error(&self) -> &Error {
        match self {
            Self::Returned { error, .. } | Self::Retained { error, .. } => error,
        }
    }

    /// Returns the descriptor when the poller released it.
    pub const fn descriptor(&self) -> Option<&OwnedFd> {
        match self {
            Self::Returned { descriptor, .. } => Some(descriptor),
            Self::Retained { .. } => None,
        }
    }

    /// Returns the attempted handle when no descriptor was returned.
    pub const fn registration(&self) -> Option<Registration> {
        match self {
            Self::Returned { .. } => None,
            Self::Retained { registration, .. } => Some(*registration),
        }
    }
}

impl fmt::Display for DeleteOwnedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error().fmt(formatter)
    }
}

impl std::error::Error for DeleteOwnedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error())
    }
}

impl AsRef<Error> for DeleteOwnedError {
    fn as_ref(&self) -> &Error {
        self.error()
    }
}

#[cfg(test)]
#[path = "delete_owned_test.rs"]
mod tests;
