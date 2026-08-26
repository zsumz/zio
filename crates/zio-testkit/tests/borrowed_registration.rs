//! Borrowed registration lifetime evidence across mutation failures.

#![allow(
    unsafe_code,
    reason = "each borrowed source remains live through proven registration retirement"
)]

use std::{error::Error as StdError, io, io::Read, io::Write, os::unix::net::UnixStream};

use zio::{
    ArmState, CommitStatus, Error, Interest, Key, Mode, Registration, RegistrationId,
    RegistrationState,
};
use zio_testkit::support::{MutationOutcome, MutationStep, ScriptedPoll};

const KEY: Key = Key::new(811);
const INTEREST: Interest = Interest::READABLE;
const MODE: Mode = Mode::OneShot;
type TestResult = Result<(), Box<dyn StdError>>;

#[test]
fn borrowed_register_failures_preserve_exact_lifetime_state() -> TestResult {
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
fn borrowed_delete_failures_preserve_exact_lifetime_state() -> TestResult {
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
    let (mut source, mut peer) = UnixStream::pair()?;
    let mut steps = vec![MutationStep::Register(failure(commit))];
    if commit == CommitStatus::NotApplied {
        steps.push(MutationStep::Register(MutationOutcome::Success));
    }
    steps.push(MutationStep::Delete(MutationOutcome::Success));
    let mut poll = ScriptedPoll::with_capacity(1, steps)?;

    // SAFETY: `source` remains open and uniquely borrowed by this poller until
    // every retained registration is deleted below.
    let result = unsafe { poll.register_borrowed(&source, KEY, INTEREST, MODE) };
    let first_id = registered_call_id(&poll, 0)?;
    let retained = failed_register(result, commit, first_id)?;
    prove_source_open(&mut source, &mut peer)?;

    match commit {
        CommitStatus::NotApplied => {
            ensure(retained.is_none(), "not-applied register retained a handle")?;
            // SAFETY: the first registration was proven absent and the source
            // remains open through deletion of this retry.
            let retry = unsafe { poll.register_borrowed(&source, KEY, INTEREST, MODE) }?;
            ensure(retry.id() != first_id, "register retry reused a generation")?;
            expect_state(&poll, &retry, armed())?;
            poll.delete(retry)?;
        }
        CommitStatus::Applied => {
            let registration = require_registration(retained)?;
            expect_state(&poll, &registration, armed())?;
            poll.delete(registration)?;
        }
        CommitStatus::Unknown => {
            let registration = require_registration(retained)?;
            expect_state(&poll, &registration, RegistrationState::Uncertain)?;
            poll.delete(registration)?;
        }
    }

    prove_source_open(&mut source, &mut peer)?;
    poll.finish()?;
    Ok(())
}

fn verify_delete_failure(commit: CommitStatus) -> TestResult {
    let (mut source, mut peer) = UnixStream::pair()?;
    let mut steps = vec![
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Delete(failure(commit)),
    ];
    if commit != CommitStatus::Applied {
        steps.push(MutationStep::Delete(MutationOutcome::Success));
    }
    let mut poll = ScriptedPoll::with_capacity(1, steps)?;

    // SAFETY: `source` remains open and uniquely borrowed by this poller until
    // the registration is proven retired below.
    let registration = unsafe { poll.register_borrowed(&source, KEY, INTEREST, MODE) }?;
    let id = registration.id();
    prove_source_open(&mut source, &mut peer)?;
    let returned = failed_delete(poll.delete(registration), commit, id)?;

    match commit {
        CommitStatus::NotApplied => {
            expect_state(&poll, &returned, armed())?;
            prove_source_open(&mut source, &mut peer)?;
            poll.delete(returned)?;
        }
        CommitStatus::Applied => expect_stale(&poll, &returned, id)?,
        CommitStatus::Unknown => {
            expect_state(&poll, &returned, RegistrationState::Uncertain)?;
            prove_source_open(&mut source, &mut peer)?;
            poll.delete(returned)?;
        }
    }

    expect_stale(&poll, &returned, id)?;
    prove_source_open(&mut source, &mut peer)?;
    poll.finish()?;
    Ok(())
}

fn failed_register(
    result: Result<Registration, zio::RegisterError>,
    expected: CommitStatus,
    id: RegistrationId,
) -> Result<Option<Registration>, Box<dyn StdError>> {
    let Err(error) = result else {
        return Err(io::Error::other("borrowed registration unexpectedly succeeded").into());
    };
    expect_commit(error.error(), expected)?;
    let registration = error.registration().copied();
    match registration {
        Some(registration) if registration.id() != id => {
            Err(io::Error::other("register failure returned another generation").into())
        }
        registration => Ok(registration),
    }
}

fn failed_delete(
    result: Result<(), zio::DeleteError>,
    expected: CommitStatus,
    id: RegistrationId,
) -> Result<Registration, Box<dyn StdError>> {
    let Err(error) = result else {
        return Err(io::Error::other("borrowed deletion unexpectedly succeeded").into());
    };
    expect_commit(error.error(), expected)?;
    let returned = *error.registration();
    ensure(
        returned.id() == id,
        "delete failure returned another generation",
    )?;
    Ok(returned)
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

fn registered_call_id(poll: &ScriptedPoll, index: usize) -> Result<RegistrationId, io::Error> {
    match poll.calls().get(index) {
        Some(zio::test_support::MutationCall::Register { registration, .. }) => Ok(*registration),
        actual => Err(io::Error::other(format!(
            "expected register call at {index}, observed {actual:?}"
        ))),
    }
}

fn expect_state(
    poll: &ScriptedPoll,
    registration: &Registration,
    expected: RegistrationState,
) -> TestResult {
    let actual = poll.registration_state(registration)?;
    ensure(actual == expected, "registration state diverged")
}

fn expect_stale(
    poll: &ScriptedPoll,
    registration: &Registration,
    id: RegistrationId,
) -> TestResult {
    match poll.registration_state(registration) {
        Err(Error::Stale { registration }) if registration == id => Ok(()),
        actual => Err(io::Error::other(format!(
            "expected stale registration {id:?}, observed {actual:?}"
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

fn require_registration(registration: Option<Registration>) -> Result<Registration, io::Error> {
    registration.ok_or_else(|| io::Error::other("register failure omitted its retained handle"))
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
