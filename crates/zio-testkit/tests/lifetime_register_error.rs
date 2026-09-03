//! Registration-error access preserves copyable lifetime evidence.

use std::{error::Error as StdError, fmt::Debug, io, os::unix::net::UnixStream};

use zio::{
    ArmState, CommitStatus, Error, Interest, Key, Mode, Operation, RegisterError, Registration,
    RegistrationId, RegistrationState,
};
use zio_testkit::support::{MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll};

const KEY: Key = Key::new(803);

#[test]
fn register_error_handles_can_be_copied_before_error_consumption() -> Result<(), Box<dyn StdError>>
{
    verify_error_copy(CommitStatus::Applied)?;
    verify_error_copy(CommitStatus::Unknown)?;
    Ok(())
}

fn verify_error_copy(commit: CommitStatus) -> Result<(), Box<dyn StdError>> {
    let source = UnixStream::pair()?.0;
    let mut poll = ScriptedPoll::with_capacity(
        1,
        [
            MutationStep::Register(MutationOutcome::Failure {
                commit,
                kind: io::ErrorKind::PermissionDenied,
            }),
            MutationStep::Delete(MutationOutcome::Success),
        ],
    )?;
    let error = register_error(poll.register(&source, KEY, Interest::READABLE, Mode::Level))?;
    let retained = error
        .registration()
        .ok_or_else(|| io::Error::other("register error omitted its retained handle"))?;
    let second_copy = retained;
    let id = retained.id();
    let (cause, returned) = error.into_parts();
    let returned = returned
        .ok_or_else(|| io::Error::other("consumed register error omitted its retained handle"))?;

    expect_register_mutation(cause, commit)?;
    check_eq(&retained, &returned, "accessed and returned handles")?;
    check_eq(&second_copy, &returned, "independent error handle copy")?;
    let expected_state = match commit {
        CommitStatus::Applied => armed(),
        CommitStatus::Unknown => RegistrationState::Uncertain,
        CommitStatus::NotApplied => {
            return Err(io::Error::other("not-applied errors do not retain handles").into());
        }
    };
    check_eq(
        &poll.registration_state(&retained)?,
        &expected_state,
        "accessed handle state",
    )?;
    check_eq(
        &poll.registration_state(&returned)?,
        &expected_state,
        "returned handle state",
    )?;
    check_eq(
        &poll.backend_state(id),
        &expected_backend(commit),
        "register error backend state",
    )?;

    poll.delete(second_copy)?;
    expect_stale(poll.registration_state(&retained), id)?;
    expect_stale(poll.registration_state(&returned), id)?;
    check_eq(&poll.calls().len(), &2, "register error call count")?;
    check_eq(
        &poll.backend_state(id),
        &ScriptedBackendState::Absent,
        "register error cleanup state",
    )?;
    poll.finish()?;
    Ok(())
}

fn register_error(result: Result<Registration, RegisterError>) -> Result<RegisterError, io::Error> {
    match result {
        Err(error) => Ok(error),
        Ok(actual) => Err(io::Error::other(format!(
            "registration unexpectedly succeeded as {:?}",
            actual.id()
        ))),
    }
}

fn expect_register_mutation(cause: Error, expected_commit: CommitStatus) -> Result<(), io::Error> {
    match cause {
        Error::Mutation(mutation) => {
            check_eq(
                &mutation.operation(),
                &Operation::Register,
                "register operation",
            )?;
            check_eq(&mutation.commit(), &expected_commit, "register commit")
        }
        actual => Err(failure("Mutation error", actual)),
    }
}

fn expect_stale(
    result: Result<RegistrationState, Error>,
    expected: RegistrationId,
) -> Result<(), io::Error> {
    match result {
        Err(Error::Stale { registration }) => {
            check_eq(&registration, &expected, "stale error handle copy")
        }
        actual => Err(failure("Stale state error", actual)),
    }
}

const fn armed() -> RegistrationState {
    RegistrationState::Registered {
        arm: ArmState::Armed,
    }
}

const fn expected_backend(commit: CommitStatus) -> ScriptedBackendState {
    match commit {
        CommitStatus::Applied => ScriptedBackendState::Registered {
            interest: Interest::READABLE,
            mode: Mode::Level,
            arm: ArmState::Armed,
        },
        CommitStatus::Unknown => ScriptedBackendState::Unknown,
        CommitStatus::NotApplied => ScriptedBackendState::Absent,
    }
}

fn check_eq<T: Debug + PartialEq + ?Sized>(
    actual: &T,
    expected: &T,
    context: &str,
) -> Result<(), io::Error> {
    if actual == expected {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{context}: expected {expected:?}, observed {actual:?}"
        )))
    }
}

fn failure(expected: impl Debug, actual: impl Debug) -> io::Error {
    io::Error::other(format!("expected {expected:?}, observed {actual:?}"))
}
