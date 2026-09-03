//! Owned-registration capability failures.

use std::{fmt, os::fd::OwnedFd};

use crate::Registration;

use super::Error;

/// Owned registration failure with its exact returned capability.
#[derive(Debug)]
pub enum RegisterOwnedError {
    /// The poller returned the original descriptor.
    Returned {
        /// Underlying failure.
        error: Error,
        /// Original descriptor.
        descriptor: OwnedFd,
    },
    /// The poller retained the descriptor under this registration.
    Retained {
        /// Underlying failure.
        error: Error,
        /// Handle for the retained registration.
        registration: Registration,
    },
}

impl RegisterOwnedError {
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

    /// Returns the descriptor when the poller did not retain it.
    pub const fn descriptor(&self) -> Option<&OwnedFd> {
        match self {
            Self::Returned { descriptor, .. } => Some(descriptor),
            Self::Retained { .. } => None,
        }
    }

    /// Returns the handle when the poller retained the descriptor.
    pub const fn registration(&self) -> Option<Registration> {
        match self {
            Self::Returned { .. } => None,
            Self::Retained { registration, .. } => Some(*registration),
        }
    }
}

impl fmt::Display for RegisterOwnedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error().fmt(formatter)
    }
}

impl std::error::Error for RegisterOwnedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error())
    }
}

impl AsRef<Error> for RegisterOwnedError {
    fn as_ref(&self) -> &Error {
        self.error()
    }
}

#[cfg(test)]
#[path = "register_owned_test.rs"]
mod tests;
