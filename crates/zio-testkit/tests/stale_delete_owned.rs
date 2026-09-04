//! Stale owned-deletion capability conformance evidence.

use std::{error::Error as StdError, fmt::Debug, io, os::unix::net::UnixStream};

use zio::{
    ArmState, CommitStatus, DeleteError, DeleteOwnedError, Error, Interest, Key, Mode, Operation,
    Registration, RegistrationId, RegistrationState,
};
use zio_testkit::support::{
    MutationCall, MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll,
};

const RETIRED_KEY: Key = Key::new(711);
const REPLACEMENT_KEY: Key = Key::new(712);

#[test]
fn stale_delete_owned_cannot_target_reused_generation() -> Result<(), Box<dyn StdError>> {
    let source = UnixStream::pair()?.0;
    let mut poll = ScriptedPoll::with_capacity(
        1,
        [
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Delete(MutationOutcome::Failure {
                commit: CommitStatus::Applied,
                kind: io::ErrorKind::BrokenPipe,
            }),
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Delete(MutationOutcome::Success),
        ],
    )?;
    let retired = poll.register(&source, RETIRED_KEY, Interest::READABLE, Mode::OneShot)?;
    let retired_id = retired.id();
    let stale = applied_delete_handle(delete_error(poll.delete(retired))?, retired_id)?;

    let replacement = poll.register(&source, REPLACEMENT_KEY, Interest::WRITABLE, Mode::Level)?;
    let replacement_id = replacement.id();
    require(
        replacement_id != retired_id,
        "slot reuse retained the retired generation",
    )?;
    let replacement_state = RegistrationState::Registered {
        arm: ArmState::Armed,
    };
    let replacement_backend = ScriptedBackendState::Registered {
        interest: Interest::WRITABLE,
        mode: Mode::Level,
        arm: ArmState::Armed,
    };

    let calls_before = poll.calls().len();
    let returned = stale_owned_handle(delete_owned_error(poll.delete_owned(stale))?, stale)?;
    check_eq(&returned, &stale, "returned stale capability")?;
    check_eq(&poll.calls().len(), &calls_before, "backend call count")?;
    check_eq(
        &poll.registration_state(&replacement)?,
        &replacement_state,
        "replacement state",
    )?;
    check_eq(
        &poll.backend_state(replacement_id),
        &replacement_backend,
        "replacement backend state",
    )?;
    check_eq(
        poll.calls(),
        expected_calls(retired_id, replacement_id).as_slice(),
        "calls before cleanup",
    )?;

    poll.delete(replacement)?;
    check_eq(
        &poll.backend_state(replacement_id),
        &ScriptedBackendState::Absent,
        "replacement backend state after cleanup",
    )?;
    poll.finish()?;
    Ok(())
}

fn applied_delete_handle(
    error: DeleteError,
    expected: RegistrationId,
) -> Result<Registration, Box<dyn StdError>> {
    let (cause, registration) = error.into_parts();
    check_eq(&registration.id(), &expected, "retired capability")?;
    match cause {
        Error::Mutation(mutation) => {
            check_eq(
                &mutation.operation(),
                &Operation::Delete,
                "delete operation",
            )?;
            check_eq(&mutation.commit(), &CommitStatus::Applied, "delete commit")?;
            check_eq(
                &mutation.source().kind(),
                &io::ErrorKind::BrokenPipe,
                "delete source kind",
            )?;
        }
        actual => return Err(failure("delete cause", "Mutation", actual).into()),
    }
    Ok(registration)
}

fn stale_owned_handle(
    error: DeleteOwnedError,
    expected: Registration,
) -> Result<Registration, Box<dyn StdError>> {
    match error {
        DeleteOwnedError::Retained {
            error: Error::Stale { registration },
            registration: returned,
        } => {
            check_eq(&registration, &expected.id(), "stale error identity")?;
            check_eq(&returned, &expected, "stale returned capability")?;
            Ok(returned)
        }
        actual => Err(failure("stale delete_owned result", "Retained(Stale)", actual).into()),
    }
}

fn delete_error(result: Result<(), DeleteError>) -> Result<DeleteError, io::Error> {
    result
        .err()
        .ok_or_else(|| io::Error::other("deletion unexpectedly succeeded"))
}

fn delete_owned_error(
    result: Result<std::os::fd::OwnedFd, DeleteOwnedError>,
) -> Result<DeleteOwnedError, io::Error> {
    result
        .err()
        .ok_or_else(|| io::Error::other("owned deletion unexpectedly succeeded"))
}

fn expected_calls(retired: RegistrationId, replacement: RegistrationId) -> [MutationCall; 3] {
    [
        MutationCall::Register {
            registration: retired,
            key: RETIRED_KEY,
            interest: Interest::READABLE,
            mode: Mode::OneShot,
        },
        MutationCall::Delete {
            registration: retired,
            interest: Interest::READABLE,
            state: RegistrationState::Registered {
                arm: ArmState::Armed,
            },
        },
        MutationCall::Register {
            registration: replacement,
            key: REPLACEMENT_KEY,
            interest: Interest::WRITABLE,
            mode: Mode::Level,
        },
    ]
}

fn check_eq<T: Debug + PartialEq + ?Sized>(
    actual: &T,
    expected: &T,
    context: &str,
) -> Result<(), io::Error> {
    if actual == expected {
        Ok(())
    } else {
        Err(failure(context, expected, actual))
    }
}

fn require(condition: bool, message: &str) -> Result<(), io::Error> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message))
    }
}

fn failure(context: &str, expected: impl Debug, actual: impl Debug) -> io::Error {
    io::Error::other(format!("{context}: expected {expected:?}, got {actual:?}"))
}
