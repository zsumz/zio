//! Mutation failure detail regressions.

use std::io;

use super::{CommitStatus, MutationError, Operation};

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
