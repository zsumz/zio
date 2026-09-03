//! Mutation failure detail regressions.

use std::io;

use super::{CommitStatus, Error, MutationError, Operation};

#[test]
fn mutation_failure_returns_every_owned_part() {
    let failure = MutationError::new(
        Operation::Modify,
        CommitStatus::Unknown,
        io::Error::from_raw_os_error(5),
    );

    let (operation, commit, source) = failure.into_parts();
    assert_eq!(operation, Operation::Modify);
    assert_eq!(commit, CommitStatus::Unknown);
    assert_eq!(source.raw_os_error(), Some(5));
}

#[test]
fn top_level_accessors_expose_embedded_diagnostics() {
    let io = Error::Io {
        operation: Operation::Wait,
        source: io::Error::from_raw_os_error(5),
    };
    assert_eq!(io.operation(), Some(Operation::Wait));
    assert_eq!(io.commit(), None);
    assert_eq!(io.io_error().and_then(io::Error::raw_os_error), Some(5));

    let mutation = Error::Mutation(MutationError::new(
        Operation::Delete,
        CommitStatus::Unknown,
        io::Error::from_raw_os_error(6),
    ));
    assert_eq!(mutation.operation(), Some(Operation::Delete));
    assert_eq!(mutation.commit(), Some(CommitStatus::Unknown));
    assert_eq!(
        mutation.io_error().and_then(io::Error::raw_os_error),
        Some(6)
    );

    assert_eq!(
        Error::UnsupportedPlatform.operation(),
        Some(Operation::UnsupportedPlatform)
    );
    assert!(Error::Invariant.io_error().is_none());
}
