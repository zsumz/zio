//! Terminal registration identity and ownership conformance evidence.

use std::{error::Error as StdError, fmt::Debug, io, os::unix::net::UnixStream};

use zio::{
    ArmState, CommitStatus, DeleteError, Error, Interest, Key, Mode, Operation, Registration,
    RegistrationId, RegistrationState,
};
use zio_testkit::support::{
    MutationCall, MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll,
};

const RETIRED_KEY: Key = Key::new(701);
const REPLACEMENT_KEY: Key = Key::new(702);

#[test]
fn delete_applied_stale_handle_survives_slot_reuse() -> Result<(), Box<dyn StdError>> {
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
    check_eq(
        &poll.backend_state(retired_id),
        &ScriptedBackendState::Absent,
        "applied deletion backend state",
    )?;

    let replacement = poll.register(&source, REPLACEMENT_KEY, Interest::WRITABLE, Mode::Level)?;
    let replacement_id = replacement.id();
    require(
        replacement_id != retired_id,
        "capacity-one slot reuse retained the retired generation",
    )?;
    let replacement_state = RegistrationState::Registered {
        arm: ArmState::Armed,
    };
    let replacement_backend = ScriptedBackendState::Registered {
        interest: Interest::WRITABLE,
        mode: Mode::Level,
        arm: ArmState::Armed,
    };
    check_eq(
        &poll.registration_state(&replacement)?,
        &replacement_state,
        "replacement portable state before stale operations",
    )?;
    check_eq(
        &poll.backend_state(replacement_id),
        &replacement_backend,
        "replacement backend state before stale operations",
    )?;

    let calls_before_rejections = poll.calls().len();
    stale_state_error(poll.registration_state(&stale), retired_id)?;
    check_call_count(&poll, calls_before_rejections, "stale state lookup")?;
    stale_error(
        poll.modify(&stale, Interest::READABLE, Mode::Level),
        retired_id,
        "stale modify",
    )?;
    check_call_count(&poll, calls_before_rejections, "stale modify")?;
    let returned_stale = stale_delete_handle(delete_error(poll.delete(stale))?, retired_id)?;
    check_eq(
        &returned_stale.id(),
        &retired_id,
        "stale delete returned capability",
    )?;
    check_call_count(&poll, calls_before_rejections, "stale delete")?;
    check_eq(
        &poll.registration_state(&replacement)?,
        &replacement_state,
        "replacement portable state after stale operations",
    )?;
    check_eq(
        &poll.backend_state(replacement_id),
        &replacement_backend,
        "replacement backend state after stale operations",
    )?;
    check_eq(
        &poll.backend_state(retired_id),
        &ScriptedBackendState::Absent,
        "retired backend state after slot reuse",
    )?;
    check_eq(
        poll.calls(),
        expected_reuse_calls(retired_id, replacement_id).as_slice(),
        "calls before replacement cleanup",
    )?;

    poll.delete(replacement)?;
    check_eq(&poll.calls().len(), &4, "calls after replacement cleanup")?;
    let cleanup_call = poll
        .calls()
        .get(3)
        .ok_or_else(|| io::Error::other("missing replacement cleanup call"))?;
    check_eq(
        cleanup_call,
        &expected_replacement_delete_call(replacement_id),
        "replacement cleanup call",
    )?;
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
    check_eq(
        &registration.id(),
        &expected,
        "applied delete returned capability",
    )?;
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
        actual => return Err(failure("applied delete cause", "Mutation", actual).into()),
    }
    Ok(registration)
}

fn stale_delete_handle(
    error: DeleteError,
    expected: RegistrationId,
) -> Result<Registration, Box<dyn StdError>> {
    let (cause, registration) = error.into_parts();
    check_eq(
        &registration.id(),
        &expected,
        "stale delete returned capability",
    )?;
    stale_error(Err(cause), expected, "stale delete")?;
    Ok(registration)
}

fn delete_error(result: Result<(), DeleteError>) -> Result<DeleteError, io::Error> {
    match result {
        Err(error) => Ok(error),
        Ok(()) => Err(io::Error::other("deletion unexpectedly succeeded")),
    }
}

fn stale_state_error(
    result: Result<RegistrationState, Error>,
    expected: RegistrationId,
) -> Result<(), io::Error> {
    match result {
        Err(error) => stale_error(Err(error), expected, "stale state lookup"),
        Ok(actual) => Err(failure("stale state lookup", "Stale", actual)),
    }
}

fn stale_error(
    result: Result<(), Error>,
    expected: RegistrationId,
    context: &str,
) -> Result<(), io::Error> {
    match result {
        Err(Error::Stale { registration }) => check_eq(&registration, &expected, context),
        Err(actual) => Err(failure(context, "Stale", actual)),
        Ok(()) => Err(io::Error::other(format!(
            "{context} unexpectedly succeeded"
        ))),
    }
}

fn check_call_count(poll: &ScriptedPoll, expected: usize, context: &str) -> Result<(), io::Error> {
    check_eq(&poll.calls().len(), &expected, context)
}

fn expected_reuse_calls(retired: RegistrationId, replacement: RegistrationId) -> [MutationCall; 3] {
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

fn expected_replacement_delete_call(registration: RegistrationId) -> MutationCall {
    MutationCall::Delete {
        registration,
        interest: Interest::WRITABLE,
        state: RegistrationState::Registered {
            arm: ArmState::Armed,
        },
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

fn failure(expected_context: &str, expected: impl Debug, actual: impl Debug) -> io::Error {
    io::Error::other(format!(
        "{expected_context}: expected {expected:?}, got {actual:?}"
    ))
}
