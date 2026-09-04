//! Owned mutations retain exactly one caller-visible capability.

use std::{
    error::Error as StdError,
    io::{self, Read, Write},
    os::fd::{AsFd, AsRawFd, OwnedFd, RawFd},
    os::unix::net::UnixStream,
};

use zio::{
    ArmState, CapacityKind, CapacityReason, CommitStatus, DeleteOwnedError, Error, Interest, Key,
    Mode, RegisterOwnedError, RegistrationState,
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
        verify_register_failure(commit)?;
    }
    Ok(())
}

#[test]
fn owned_capacity_failure_returns_exact_descriptor_without_backend_work() -> TestResult {
    let filler = UnixStream::pair()?.0;
    let (source, peer) = UnixStream::pair()?;
    let descriptor: OwnedFd = source.into();
    let raw = descriptor.as_raw_fd();
    let mut poll = ScriptedPoll::with_capacity(
        1,
        [
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Delete(MutationOutcome::Success),
        ],
    )?;
    let retained = poll.register(&filler, Key::new(820), Interest::READABLE, Mode::Level)?;
    let calls = poll.calls().len();

    let Err(error) = poll.register_owned(descriptor, KEY, Interest::WRITABLE, Mode::OneShot) else {
        return Err("owned registration unexpectedly exceeded capacity".into());
    };
    let descriptor = match error {
        RegisterOwnedError::Returned {
            error:
                Error::Capacity {
                    kind: CapacityKind::Registration,
                    limit: 1,
                    reason: CapacityReason::Exhausted,
                    ..
                },
            descriptor,
        } => descriptor,
        actual => {
            return Err(io::Error::other(format!(
                "expected returned capacity failure, observed {actual:?}"
            ))
            .into());
        }
    };

    assert_eq!(poll.calls().len(), calls);
    assert_eq!(poll.registration_count(), 1);
    assert_eq!(
        poll.registration_state(&retained)?,
        RegistrationState::Registered {
            arm: ArmState::Armed,
        }
    );
    prove_descriptor_open(descriptor, peer, raw)?;

    poll.delete(retained)?;
    poll.finish()?;
    Ok(())
}

#[test]
fn owned_invalid_interest_returns_exact_descriptor_without_backend_work() -> TestResult {
    let (source, peer) = UnixStream::pair()?;
    let descriptor: OwnedFd = source.into();
    let raw = descriptor.as_raw_fd();
    let mut poll = ScriptedPoll::new(std::iter::empty::<MutationStep>())?;

    let Err(error) = poll.register_owned(descriptor, KEY, Interest::EMPTY, Mode::Level) else {
        return Err("owned registration accepted empty interest".into());
    };
    let descriptor = match error {
        RegisterOwnedError::Returned {
            error: Error::InvalidInterest,
            descriptor,
        } => descriptor,
        actual => {
            return Err(io::Error::other(format!(
                "expected returned invalid-interest failure, observed {actual:?}"
            ))
            .into());
        }
    };

    assert!(poll.calls().is_empty());
    assert_eq!(poll.registration_count(), 0);
    prove_descriptor_open(descriptor, peer, raw)?;
    poll.finish()?;
    Ok(())
}

#[test]
fn owned_delete_failures_return_the_exact_capability() -> TestResult {
    for commit in [
        CommitStatus::NotApplied,
        CommitStatus::Applied,
        CommitStatus::Unknown,
    ] {
        verify_delete_failure(commit)?;
    }
    Ok(())
}

fn verify_register_failure(commit: CommitStatus) -> TestResult {
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

fn verify_delete_failure(commit: CommitStatus) -> TestResult {
    let (source, _peer) = UnixStream::pair()?;
    let descriptor = source.as_fd().try_clone_to_owned()?;
    let raw = descriptor.as_raw_fd();
    let mut steps = vec![
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Delete(MutationOutcome::Failure {
            commit,
            kind: io::ErrorKind::PermissionDenied,
        }),
    ];
    if commit != CommitStatus::Applied {
        steps.push(MutationStep::Delete(MutationOutcome::Success));
    }
    let mut poll = ScriptedPoll::with_capacity(1, steps)?;
    let registration = poll.register_owned(descriptor, KEY, Interest::READABLE, Mode::Level)?;

    let Err(error) = poll.delete_owned(registration) else {
        return Err("planned owned deletion failure succeeded".into());
    };
    assert_eq!(error.error().commit(), Some(commit));
    match (commit, error) {
        (CommitStatus::Applied, DeleteOwnedError::Returned { descriptor, .. }) => {
            assert_eq!(descriptor.as_raw_fd(), raw);
            assert_eq!(poll.registration_count(), 0);
        }
        (
            CommitStatus::NotApplied | CommitStatus::Unknown,
            DeleteOwnedError::Retained {
                registration: returned,
                ..
            },
        ) => {
            assert_eq!(returned, registration);
            assert_eq!(poll.registration_fd(&returned)?.as_raw_fd(), raw);
            let expected = match commit {
                CommitStatus::NotApplied => RegistrationState::Registered {
                    arm: ArmState::Armed,
                },
                CommitStatus::Unknown => RegistrationState::Uncertain,
                CommitStatus::Applied => return Err(Error::Invariant.into()),
            };
            assert_eq!(poll.registration_state(&returned)?, expected);
            let descriptor = poll.delete_owned(returned)?;
            assert_eq!(descriptor.as_raw_fd(), raw);
        }
        _ => return Err(Error::Invariant.into()),
    }
    assert!(matches!(
        poll.registration_state(&registration),
        Err(Error::Stale { registration: stale }) if stale == registration.id()
    ));
    poll.finish()?;
    Ok(())
}

fn prove_descriptor_open(
    descriptor: OwnedFd,
    mut peer: UnixStream,
    expected_raw: RawFd,
) -> TestResult {
    assert_eq!(descriptor.as_raw_fd(), expected_raw);
    let mut returned = UnixStream::from(descriptor);
    peer.write_all(b"z")?;
    let mut byte = [0_u8; 1];
    returned.read_exact(&mut byte)?;
    assert_eq!(byte, *b"z");
    Ok(())
}
