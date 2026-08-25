//! Exact setup and selector-mutation failures.

use std::io;

use crate::error::{CommitStatus, Operation};

/// A selector mutation failure with a proven commit status.
#[derive(Debug)]
pub(crate) struct MutationFailure {
    commit: CommitStatus,
    source: io::Error,
}

impl MutationFailure {
    pub(crate) const fn new(commit: CommitStatus, source: io::Error) -> Self {
        Self { commit, source }
    }

    pub(crate) const fn commit(&self) -> CommitStatus {
        self.commit
    }

    pub(crate) fn into_source(self) -> io::Error {
        self.source
    }
}

/// A selector-construction failure retaining its precise operation.
#[derive(Debug)]
pub(crate) struct SetupFailure {
    operation: Operation,
    source: io::Error,
}

impl SetupFailure {
    pub(crate) const fn new(operation: Operation, source: io::Error) -> Self {
        Self { operation, source }
    }

    pub(crate) const fn operation(&self) -> Operation {
        self.operation
    }

    pub(crate) fn into_source(self) -> io::Error {
        self.source
    }
}
