//! Owned recovery outcome and failure regressions.

use std::io;

use crate::{
    ArmState, CommitStatus, Operation, Registration, RegistrationState,
    error::{RecoveryFailure, RecoveryOutcome},
};

#[test]
fn recovery_outcomes_enforce_commit_state_pairs() {
    let applied = RecoveryOutcome::new(registration(11), CommitStatus::Applied);
    let not_applied = RecoveryOutcome::new(registration(12), CommitStatus::NotApplied);
    let unknown = RecoveryOutcome::new(registration(13), CommitStatus::Unknown);

    assert_eq!(applied.registration(), registration(11));
    assert_eq!(applied.commit(), CommitStatus::Applied);
    assert_eq!(
        applied.state(),
        RegistrationState::Registered {
            arm: ArmState::Disarmed,
        }
    );
    assert_eq!(not_applied.commit(), CommitStatus::NotApplied);
    assert_eq!(
        not_applied.state(),
        RegistrationState::Registered {
            arm: ArmState::Armed,
        }
    );
    assert_eq!(unknown.commit(), CommitStatus::Unknown);
    assert_eq!(unknown.state(), RegistrationState::Uncertain);
}

#[test]
fn recovery_failure_preserves_one_owned_snapshot() {
    let outcomes = vec![
        RecoveryOutcome::new(registration(21), CommitStatus::Applied),
        RecoveryOutcome::new(registration(22), CommitStatus::Unknown),
    ];
    let pointer = outcomes.as_ptr();
    let capacity = outcomes.capacity();
    assert_eq!(capacity, outcomes.len());
    let failure =
        RecoveryFailure::new(Operation::Disarm, outcomes, io::Error::from_raw_os_error(5));

    assert_eq!(failure.operation(), Operation::Disarm);
    assert_eq!(failure.outcomes().as_ptr(), pointer);
    assert_eq!(failure.outcomes().len(), 2);
    assert_eq!(failure.len(), 2);
    assert!(!failure.is_empty());
    assert_eq!(failure.as_ref(), failure.outcomes());
    assert_eq!(failure.iter().len(), 2);
    assert_eq!(failure.into_iter().len(), 2);
    assert_eq!(failure.source().raw_os_error(), Some(5));
    assert_eq!(
        failure.to_string(),
        format!(
            "disarm recovery failed for 2 registrations: {}",
            failure.source()
        )
    );

    let (operation, outcomes, source) = failure.into_parts();
    assert_eq!(operation, Operation::Disarm);
    assert_eq!(outcomes.as_ptr(), pointer);
    assert_eq!(outcomes.capacity(), capacity);
    assert_eq!(source.raw_os_error(), Some(5));
}

#[test]
fn retained_recovery_snapshots_are_independent() {
    let first = RecoveryFailure::new(
        Operation::Disarm,
        vec![RecoveryOutcome::new(
            registration(31),
            CommitStatus::NotApplied,
        )],
        io::Error::from_raw_os_error(5),
    );
    let second = RecoveryFailure::new(
        Operation::Disarm,
        vec![RecoveryOutcome::new(
            registration(32),
            CommitStatus::Unknown,
        )],
        io::Error::from_raw_os_error(6),
    );

    assert_ne!(first.outcomes().as_ptr(), second.outcomes().as_ptr());
    assert_eq!(first.outcomes()[0].registration(), registration(31));
    assert_eq!(second.outcomes()[0].registration(), registration(32));
    assert_eq!(first.source().raw_os_error(), Some(5));
    assert_eq!(second.source().raw_os_error(), Some(6));
    assert_eq!(
        first.to_string(),
        format!(
            "disarm recovery failed for 1 registration: {}",
            first.source()
        )
    );
}

const fn registration(id: u64) -> Registration {
    Registration::test(id)
}
