//! Borrowed modification lifetime evidence across mutation failures.

#![allow(
    unsafe_code,
    reason = "each borrowed source remains live through proven registration retirement"
)]

use std::{error::Error as StdError, io, io::Read, io::Write, os::unix::net::UnixStream};

use zio::{ArmState, CommitStatus, Error, Interest, Key, Mode, RegistrationState};
use zio_testkit::support::{MutationCall, MutationOutcome, MutationStep, ScriptedPoll};

const KEY: Key = Key::new(812);
const PRIOR_INTEREST: Interest = Interest::READABLE;
const PRIOR_MODE: Mode = Mode::OneShot;
const DESIRED_INTEREST: Interest = Interest::WRITABLE;
const DESIRED_MODE: Mode = Mode::Level;
type TestResult = Result<(), Box<dyn StdError>>;

#[test]
fn borrowed_modify_failures_preserve_exact_lifetime_state() -> TestResult {
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
    let (mut source, mut peer) = UnixStream::pair()?;
    let mut steps = vec![
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Modify(failure(commit)),
    ];
    if commit != CommitStatus::Unknown {
        steps.push(MutationStep::Modify(MutationOutcome::Success));
    }
    steps.push(MutationStep::Delete(MutationOutcome::Success));
    let mut poll = ScriptedPoll::with_capacity(1, steps)?;

    // SAFETY: `source` remains open and uniquely borrowed through cleanup.
    let registration = unsafe { poll.register_borrowed(&source, KEY, PRIOR_INTEREST, PRIOR_MODE)? };
    let Err(error) = poll.modify(&registration, DESIRED_INTEREST, DESIRED_MODE) else {
        return Err(io::Error::other("borrowed modification unexpectedly succeeded").into());
    };
    expect_commit(&error, commit)?;
    let state = if commit == CommitStatus::Unknown {
        RegistrationState::Uncertain
    } else {
        armed()
    };
    ensure(
        poll.registration_state(&registration)? == state,
        "registration state diverged",
    )?;
    prove_source_open(&mut source, &mut peer)?;

    if commit != CommitStatus::Unknown {
        poll.modify(&registration, DESIRED_INTEREST, DESIRED_MODE)?;
        expect_retry_prior(&poll, commit)?;
    }
    poll.delete(registration)?;
    prove_source_open(&mut source, &mut peer)?;
    poll.finish()?;
    Ok(())
}

fn expect_retry_prior(poll: &ScriptedPoll, commit: CommitStatus) -> TestResult {
    let expected = if commit == CommitStatus::NotApplied {
        (PRIOR_INTEREST, PRIOR_MODE)
    } else {
        (DESIRED_INTEREST, DESIRED_MODE)
    };
    match poll.calls().get(2) {
        Some(MutationCall::Modify {
            previous_interest,
            previous_mode,
            previous_arm: ArmState::Armed,
            ..
        }) if (*previous_interest, *previous_mode) == expected => Ok(()),
        actual => Err(io::Error::other(format!("unexpected retry state: {actual:?}")).into()),
    }
}

fn expect_commit(error: &Error, expected: CommitStatus) -> TestResult {
    match error {
        Error::Mutation(mutation) if mutation.commit() == expected => Ok(()),
        actual => Err(io::Error::other(format!(
            "expected {expected:?} mutation, observed {actual:?}"
        ))
        .into()),
    }
}

fn prove_source_open(source: &mut UnixStream, peer: &mut UnixStream) -> TestResult {
    peer.write_all(b"z")?;
    let mut byte = [0_u8; 1];
    source.read_exact(&mut byte)?;
    ensure(byte == *b"z", "borrowed source changed while retained")
}

const fn armed() -> RegistrationState {
    RegistrationState::Registered {
        arm: ArmState::Armed,
    }
}

const fn failure(commit: CommitStatus) -> MutationOutcome {
    MutationOutcome::Failure {
        commit,
        kind: io::ErrorKind::PermissionDenied,
    }
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message).into())
    }
}
