//! Key and backend-configuration settlement under one mutation outcome.

use std::{error::Error as StdError, io, os::unix::net::UnixStream};

use zio::{ArmState, CommitStatus, Error, Interest, Key, Mode, RegistrationState};
use zio_testkit::support::{MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll};

const PRIOR_KEY: Key = Key::new(31);
const DESIRED_KEY: Key = Key::new(32);
const PRIOR_INTEREST: Interest = Interest::READABLE;
const DESIRED_INTEREST: Interest = Interest::WRITABLE;
type TestResult = Result<(), Box<dyn StdError>>;

#[test]
fn key_settlement_matches_backend_commit_status() -> TestResult {
    verify(MutationOutcome::Success, None)?;
    for commit in [
        CommitStatus::NotApplied,
        CommitStatus::Applied,
        CommitStatus::Unknown,
    ] {
        verify(failure(commit), Some(commit))?;
    }
    Ok(())
}

fn verify(outcome: MutationOutcome, expected_error: Option<CommitStatus>) -> TestResult {
    let source = UnixStream::pair()?.0;
    let mut poll = ScriptedPoll::with_capacity(
        1,
        [
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Modify(outcome),
            MutationStep::Delete(MutationOutcome::Success),
        ],
    )?;
    let registration = poll.register(&source, PRIOR_KEY, PRIOR_INTEREST, Mode::Level)?;

    let result = poll.modify_with_key(&registration, DESIRED_KEY, DESIRED_INTEREST, Mode::OneShot);
    match (expected_error, result) {
        (None, Ok(())) => {}
        (Some(expected), Err(Error::Mutation(error))) if error.commit() == expected => {}
        (expected, actual) => {
            return Err(io::Error::other(format!(
                "expected commit {expected:?}, observed {actual:?}"
            ))
            .into());
        }
    }

    let info = poll.registration_info(&registration)?;
    let committed = matches!(expected_error, None | Some(CommitStatus::Applied));
    assert_eq!(info.key(), if committed { DESIRED_KEY } else { PRIOR_KEY });
    assert_eq!(
        info.interest(),
        if committed {
            DESIRED_INTEREST
        } else {
            PRIOR_INTEREST
        }
    );
    assert_eq!(
        info.mode(),
        if committed {
            Mode::OneShot
        } else {
            Mode::Level
        }
    );
    assert_eq!(
        info.state(),
        if expected_error == Some(CommitStatus::Unknown) {
            RegistrationState::Uncertain
        } else {
            RegistrationState::Registered {
                arm: ArmState::Armed,
            }
        }
    );
    assert_eq!(
        poll.backend_state(registration.id()),
        expected_backend(expected_error)
    );
    poll.delete(registration)?;
    poll.finish()?;
    Ok(())
}

const fn expected_backend(commit: Option<CommitStatus>) -> ScriptedBackendState {
    match commit {
        None | Some(CommitStatus::Applied) => ScriptedBackendState::Registered {
            interest: DESIRED_INTEREST,
            mode: Mode::OneShot,
            arm: ArmState::Armed,
        },
        Some(CommitStatus::NotApplied) => ScriptedBackendState::Registered {
            interest: PRIOR_INTEREST,
            mode: Mode::Level,
            arm: ArmState::Armed,
        },
        Some(CommitStatus::Unknown) => ScriptedBackendState::Unknown,
    }
}

const fn failure(commit: CommitStatus) -> MutationOutcome {
    MutationOutcome::Failure {
        commit,
        kind: io::ErrorKind::PermissionDenied,
    }
}
