//! Owned registration retains exactly one caller-visible capability.

use std::{
    error::Error as StdError,
    io,
    os::fd::{AsFd, AsRawFd},
    os::unix::net::UnixStream,
};

use zio::{
    ArmState, CommitStatus, Error, Interest, Key, Mode, RegisterOwnedError, RegistrationState,
};
use zio_testkit::support::{MutationOutcome, MutationStep, ScriptedPoll};

const KEY: Key = Key::new(821);
type TestResult = Result<(), Box<dyn StdError>>;

#[test]
fn owned_register_failures_return_the_exact_capability() -> TestResult {
    for commit in [
        CommitStatus::NotApplied,
        CommitStatus::Applied,
        CommitStatus::Unknown,
    ] {
        verify_failure(commit)?;
    }
    Ok(())
}

fn verify_failure(commit: CommitStatus) -> TestResult {
    let (source, _peer) = UnixStream::pair()?;
    let descriptor = source.as_fd().try_clone_to_owned()?;
    let raw = descriptor.as_raw_fd();
    let mut poll = ScriptedPoll::with_capacity(
        1,
        [MutationStep::Register(MutationOutcome::Failure {
            commit,
            kind: io::ErrorKind::PermissionDenied,
        })],
    )?;

    let Err(error) = poll.register_owned(descriptor, KEY, Interest::READABLE, Mode::Level) else {
        return Err("planned owned registration failure succeeded".into());
    };
    assert_eq!(error.error().commit(), Some(commit));

    match (commit, error) {
        (CommitStatus::NotApplied, RegisterOwnedError::Returned { descriptor, .. }) => {
            assert_eq!(descriptor.as_raw_fd(), raw);
            assert_eq!(poll.registration_count(), 0);
        }
        (CommitStatus::Applied, RegisterOwnedError::Retained { registration, .. }) => {
            assert_eq!(poll.registration_fd(&registration)?.as_raw_fd(), raw);
            assert_eq!(
                poll.registration_state(&registration)?,
                RegistrationState::Registered {
                    arm: ArmState::Armed,
                }
            );
        }
        (CommitStatus::Unknown, RegisterOwnedError::Retained { registration, .. }) => {
            assert_eq!(poll.registration_fd(&registration)?.as_raw_fd(), raw);
            assert_eq!(
                poll.registration_state(&registration)?,
                RegistrationState::Uncertain
            );
        }
        _ => return Err(Error::Invariant.into()),
    }
    poll.finish()?;
    Ok(())
}
