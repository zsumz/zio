//! Cross-poller registration ownership conformance evidence.

use std::{error::Error as StdError, fmt::Debug, io, os::unix::net::UnixStream};

use zio::{
    ArmState, DeleteError, DeleteOwnedError, Error, Interest, Key, Mode, Registration,
    RegistrationId, RegistrationState,
};
use zio_testkit::support::{
    MutationCall, MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll,
};

const KEY: Key = Key::new(703);

#[test]
fn wrong_poller_delete_returns_exact_handle_to_owner() -> Result<(), Box<dyn StdError>> {
    verify_wrong_poller_delete(DeleteApi::Plain)
}

#[test]
fn wrong_poller_delete_owned_returns_exact_handle_to_owner() -> Result<(), Box<dyn StdError>> {
    verify_wrong_poller_delete(DeleteApi::Owned)
}

#[derive(Clone, Copy)]
enum DeleteApi {
    Plain,
    Owned,
}

fn verify_wrong_poller_delete(api: DeleteApi) -> Result<(), Box<dyn StdError>> {
    let source = UnixStream::pair()?.0;
    let mut owner = ScriptedPoll::with_capacity(
        1,
        [
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Delete(MutationOutcome::Success),
        ],
    )?;
    let mut stranger = ScriptedPoll::with_capacity(1, std::iter::empty::<MutationStep>())?;
    let registration = owner.register(&source, KEY, Interest::READABLE, Mode::OneShot)?;
    let id = registration.id();
    let retained_state = RegistrationState::Registered {
        arm: ArmState::Armed,
    };
    let retained_backend = ScriptedBackendState::Registered {
        interest: Interest::READABLE,
        mode: Mode::OneShot,
        arm: ArmState::Armed,
    };
    let owner_calls = owner.calls().len();

    let returned = reject_delete(&mut stranger, registration, api)?;
    check_eq(&stranger.calls().len(), &0, "stranger call count")?;
    check_eq(&owner.calls().len(), &owner_calls, "owner call count")?;
    check_eq(
        &owner.registration_state(&returned)?,
        &retained_state,
        "owner portable state after wrong-owner delete",
    )?;
    check_eq(
        &owner.backend_state(id),
        &retained_backend,
        "owner backend state after wrong-owner delete",
    )?;

    owner.delete(returned)?;
    check_eq(
        &owner.backend_state(id),
        &ScriptedBackendState::Absent,
        "owner backend state after cleanup",
    )?;
    check_eq(
        owner.calls(),
        expected_owner_calls(id).as_slice(),
        "owner calls",
    )?;
    owner.finish()?;
    stranger.finish()?;
    Ok(())
}

fn reject_delete(
    stranger: &mut ScriptedPoll,
    registration: Registration,
    api: DeleteApi,
) -> Result<Registration, Box<dyn StdError>> {
    match api {
        DeleteApi::Plain => {
            wrong_owner_handle(delete_error(stranger.delete(registration))?, registration)
        }
        DeleteApi::Owned => wrong_owner_owned_handle(
            delete_owned_error(stranger.delete_owned(registration))?,
            registration,
        ),
    }
}

fn wrong_owner_handle(
    error: DeleteError,
    expected: Registration,
) -> Result<Registration, Box<dyn StdError>> {
    let (cause, registration) = error.into_parts();
    check_eq(&registration, &expected, "wrong-owner returned capability")?;
    expect_wrong_owner(cause, expected)?;
    Ok(registration)
}

fn wrong_owner_owned_handle(
    error: DeleteOwnedError,
    expected: Registration,
) -> Result<Registration, Box<dyn StdError>> {
    let DeleteOwnedError::Retained {
        error,
        registration,
    } = error
    else {
        return Err(failure("wrong-owner owned result", "Retained", error).into());
    };
    check_eq(&registration, &expected, "wrong-owner returned capability")?;
    expect_wrong_owner(error, expected)?;
    Ok(registration)
}

fn expect_wrong_owner(cause: Error, expected: Registration) -> Result<(), io::Error> {
    match cause {
        Error::WrongPoller { registration } => {
            check_eq(&registration, &expected, "wrong-owner error identity")?;
        }
        actual => return Err(failure("wrong-owner cause", "WrongPoller", actual)),
    }
    Ok(())
}

fn delete_error(result: Result<(), DeleteError>) -> Result<DeleteError, io::Error> {
    match result {
        Err(error) => Ok(error),
        Ok(()) => Err(io::Error::other("deletion unexpectedly succeeded")),
    }
}

fn delete_owned_error(
    result: Result<std::os::fd::OwnedFd, DeleteOwnedError>,
) -> Result<DeleteOwnedError, io::Error> {
    match result {
        Err(error) => Ok(error),
        Ok(_) => Err(io::Error::other("owned deletion unexpectedly succeeded")),
    }
}

fn expected_owner_calls(registration: RegistrationId) -> [MutationCall; 2] {
    [
        MutationCall::Register {
            registration,
            key: KEY,
            interest: Interest::READABLE,
            mode: Mode::OneShot,
        },
        MutationCall::Delete {
            registration,
            interest: Interest::READABLE,
            state: RegistrationState::Registered {
                arm: ArmState::Armed,
            },
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

fn failure(expected_context: &str, expected: impl Debug, actual: impl Debug) -> io::Error {
    io::Error::other(format!(
        "{expected_context}: expected {expected:?}, got {actual:?}"
    ))
}
