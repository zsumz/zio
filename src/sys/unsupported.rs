//! Explicit unsupported-platform backend.

#![cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
)))]

use std::{io, sync::Arc};

use crate::{
    ArmState, Interest, Mode, Wait,
    error::{CommitStatus, Operation},
};

use super::{
    event::RawEvent,
    failure::{MutationFailure, SetupFailure},
};

/// Empty raw batch retained only so unsupported targets compile.
#[derive(Debug)]
pub(crate) struct RawBatch;

impl RawBatch {
    pub(crate) const fn event(&self, _index: usize, _observed: usize) -> Option<RawEvent> {
        None
    }
}

/// Wake handle that reports unsupported use.
#[derive(Debug)]
pub(crate) struct Wake;

impl Wake {
    pub(crate) fn wake(&self) -> io::Result<()> {
        Err(unsupported_error())
    }
}

/// Selector that cannot be constructed on this target.
#[derive(Debug)]
pub(crate) struct Backend;

impl Backend {
    pub(crate) fn raw_batch(capacity: usize) -> Option<RawBatch> {
        (capacity > 0).then_some(RawBatch)
    }

    pub(crate) fn new() -> Result<(Self, Arc<Wake>), SetupFailure> {
        Err(SetupFailure::new(
            Operation::UnsupportedPlatform,
            unsupported_error(),
        ))
    }

    pub(crate) fn register<F>(
        &self,
        _source: F,
        _token: u64,
        _interest: Interest,
        _mode: Mode,
    ) -> Result<(), MutationFailure> {
        Err(unsupported_mutation())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "matches the supported backend contract"
    )]
    pub(crate) fn modify<F>(
        &self,
        _source: F,
        _token: u64,
        _previous_interest: Interest,
        _previous_mode: Mode,
        _previous_arm: ArmState,
        _desired_interest: Interest,
        _desired_mode: Mode,
    ) -> Result<(), MutationFailure> {
        Err(unsupported_mutation())
    }

    pub(crate) fn delete<F>(&self, _source: F, _interest: Interest) -> Result<(), MutationFailure> {
        Err(unsupported_mutation())
    }

    pub(crate) fn wait(&self, _batch: &mut RawBatch, _wait: Wait) -> io::Result<usize> {
        Err(unsupported_error())
    }

    pub(crate) fn disarm(
        &self,
        _descriptor: i32,
        _interest: Interest,
    ) -> Result<(), MutationFailure> {
        Err(unsupported_mutation())
    }
}

fn unsupported_mutation() -> MutationFailure {
    MutationFailure::new(CommitStatus::NotApplied, unsupported_error())
}

fn unsupported_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "zio is unsupported on this platform",
    )
}
