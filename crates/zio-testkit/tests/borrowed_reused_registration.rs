//! Borrowed reused-slot registration outcome matrix.

#![allow(
    unsafe_code,
    reason = "the source remains live through proven retirement of every registration"
)]

use std::{error::Error as StdError, io, os::unix::net::UnixStream};

use zio::{
    ArmState, CommitStatus, Error, Interest, Key, Mode, Registration, RegistrationId,
    RegistrationState,
};
use zio_testkit::support::{
    MutationCall, MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll,
};

const KEY: Key = Key::new(911);
const INTEREST: Interest = Interest::READABLE;
const MODE: Mode = Mode::OneShot;
type TestResult = Result<(), Box<dyn StdError>>;

#[test]
fn reused_borrowed_registration_preserves_every_outcome() -> TestResult {
    verify_reused(Outcome::Success)?;
    for commit in [
        CommitStatus::NotApplied,
        CommitStatus::Applied,
        CommitStatus::Unknown,
    ] {
        verify_reused(Outcome::Failure(commit))?;
    }
    Ok(())
}

fn verify_reused(outcome: Outcome) -> TestResult {
    let (source, _peer) = UnixStream::pair()?;
    let mut steps = vec![
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Delete(MutationOutcome::Success),
        MutationStep::Register(outcome.script()),
    ];
    if outcome == Outcome::Failure(CommitStatus::NotApplied) {
        steps.push(MutationStep::Register(MutationOutcome::Success));
    }
    steps.push(MutationStep::Delete(MutationOutcome::Success));
    let mut poll = ScriptedPoll::with_capacity(1, steps)?;

    let seed = borrowed_register(&mut poll, &source)?;
    let seed_id = seed.id();
    poll.delete(seed)?;
    let result = unsafe { poll.register_borrowed(&source, KEY, INTEREST, MODE) };
    let reused_id = register_call_id(&poll, 2)?;
    ensure(
        reused_id != seed_id,
        "reused slot repeated its retired generation",
    )?;

    match outcome {
        Outcome::Success => {
            let registration = result?;
            ensure(
                registration.id() == reused_id,
                "success returned another generation",
            )?;
            expect_state(&poll, &registration, armed())?;
            poll.delete(registration)?;
        }
        Outcome::Failure(commit) => settle_failure(&mut poll, &source, result, reused_id, commit)?,
    }
    poll.finish()?;
    Ok(())
}

fn settle_failure(
    poll: &mut ScriptedPoll,
    source: &UnixStream,
    result: Result<Registration, zio::RegisterError>,
    reused_id: RegistrationId,
    commit: CommitStatus,
) -> TestResult {
    let error = match result {
        Ok(registration) => {
            return Err(io::Error::other(format!(
                "scripted reused registration unexpectedly returned {:?}",
                registration.id()
            ))
            .into());
        }
        Err(error) => error,
    };
    expect_commit(error.error(), commit)?;
    let retained = error.registration().copied();
    match commit {
        CommitStatus::NotApplied => {
            ensure(retained.is_none(), "not-applied reuse retained a handle")?;
            ensure(
                poll.backend_state(reused_id) == ScriptedBackendState::Absent,
                "not-applied reuse remained in the backend",
            )?;
            let retry = borrowed_register(poll, source)?;
            ensure(
                retry.id() != reused_id,
                "retry repeated the failed generation",
            )?;
            poll.delete(retry)?;
        }
        CommitStatus::Applied => {
            let registration = require_exact(retained, reused_id)?;
            expect_state(poll, &registration, armed())?;
            poll.delete(registration)?;
        }
        CommitStatus::Unknown => {
            let registration = require_exact(retained, reused_id)?;
            expect_state(poll, &registration, RegistrationState::Uncertain)?;
            poll.delete(registration)?;
        }
    }
    Ok(())
}

fn borrowed_register(
    poll: &mut ScriptedPoll,
    source: &UnixStream,
) -> Result<Registration, zio::RegisterError> {
    // SAFETY: `source` remains live and unchanged until successful deletion.
    unsafe { poll.register_borrowed(source, KEY, INTEREST, MODE) }
}

fn register_call_id(poll: &ScriptedPoll, index: usize) -> Result<RegistrationId, io::Error> {
    match poll.calls().get(index) {
        Some(MutationCall::Register { registration, .. }) => Ok(*registration),
        actual => Err(io::Error::other(format!(
            "expected register call at {index}, observed {actual:?}"
        ))),
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

fn expect_state(
    poll: &ScriptedPoll,
    registration: &Registration,
    expected: RegistrationState,
) -> TestResult {
    ensure(
        poll.registration_state(registration)? == expected,
        "portable registration state diverged",
    )
}

fn require_exact(
    registration: Option<Registration>,
    expected: RegistrationId,
) -> Result<Registration, io::Error> {
    let registration = registration
        .ok_or_else(|| io::Error::other("register failure omitted its exact handle"))?;
    if registration.id() == expected {
        Ok(registration)
    } else {
        Err(io::Error::other(
            "register failure returned another generation",
        ))
    }
}

const fn armed() -> RegistrationState {
    RegistrationState::Registered {
        arm: ArmState::Armed,
    }
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message).into())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Outcome {
    Success,
    Failure(CommitStatus),
}

impl Outcome {
    const fn script(self) -> MutationOutcome {
        match self {
            Self::Success => MutationOutcome::Success,
            Self::Failure(commit) => MutationOutcome::Failure {
                commit,
                kind: io::ErrorKind::PermissionDenied,
            },
        }
    }
}
