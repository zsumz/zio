//! Copyable registration ownership remains poller-local.

use std::{error::Error as StdError, fmt::Debug, io, os::unix::net::UnixStream};

use zio::{ArmState, DeleteError, Error, Interest, Key, Mode, Registration, RegistrationState};
use zio_testkit::support::{MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll};

const KEY: Key = Key::new(802);

#[test]
fn copied_handle_rejects_wrong_poller_without_backend_calls() -> Result<(), Box<dyn StdError>> {
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
    let state_copy = registration;
    let info_copy = registration;
    let descriptor_copy = registration;
    let key_copy = registration;
    let modify_copy = registration;
    let keyed_modify_copy = registration;
    let rearm_copy = registration;
    let delete_copy = registration;
    let cleanup_copy = registration;

    expect_wrong_poller(stranger.registration_state(&state_copy), state_copy)?;
    expect_wrong_poller(stranger.registration_info(&info_copy), info_copy)?;
    expect_wrong_poller(stranger.registration_fd(&descriptor_copy), descriptor_copy)?;
    check_eq(
        &stranger.contains(&registration),
        &false,
        "stranger membership",
    )?;
    expect_wrong_poller(stranger.set_key(&key_copy, Key::new(803)), key_copy)?;
    expect_wrong_poller(
        stranger.modify(&modify_copy, Interest::WRITABLE, Mode::Level),
        modify_copy,
    )?;
    expect_wrong_poller(
        stranger.modify_with_key(
            &keyed_modify_copy,
            Key::new(804),
            Interest::WRITABLE,
            Mode::Level,
        ),
        keyed_modify_copy,
    )?;
    expect_wrong_poller(stranger.rearm(&rearm_copy), rearm_copy)?;
    let returned = expect_wrong_poller_delete(stranger.delete(delete_copy), delete_copy)?;
    check_eq(&returned, &registration, "wrong-poller returned copy")?;
    check_eq(&stranger.calls().len(), &0, "stranger backend calls")?;
    check_eq(&owner.calls().len(), &1, "owner backend calls")?;

    check_eq(
        &owner.registration_state(&registration)?,
        &armed(),
        "owner portable state",
    )?;
    check_eq(
        &owner.backend_state(id),
        &backend_registered(),
        "owner backend state",
    )?;
    check_eq(
        &owner.registration_info(&registration)?.key(),
        &KEY,
        "owner key",
    )?;
    owner.delete(cleanup_copy)?;
    check_eq(&owner.calls().len(), &2, "owner cleanup calls")?;
    owner.finish()?;
    stranger.finish()?;
    Ok(())
}

fn expect_wrong_poller<T: Debug>(
    result: Result<T, Error>,
    expected: Registration,
) -> Result<(), io::Error> {
    match result {
        Err(Error::WrongPoller { registration }) => {
            check_eq(&registration, &expected, "wrong-poller identity")
        }
        actual => Err(failure("WrongPoller error", actual)),
    }
}

fn expect_wrong_poller_delete(
    result: Result<(), DeleteError>,
    expected: Registration,
) -> Result<Registration, Box<dyn StdError>> {
    let Err(error) = result else {
        return Err(io::Error::other("wrong-poller deletion succeeded").into());
    };
    let retained = error.registration();
    let (cause, returned) = error.into_parts();
    expect_wrong_poller::<()>(Err(cause), expected)?;
    check_eq(&retained, &returned, "wrong-poller error copies")?;
    Ok(returned)
}

const fn armed() -> RegistrationState {
    RegistrationState::Registered {
        arm: ArmState::Armed,
    }
}

const fn backend_registered() -> ScriptedBackendState {
    ScriptedBackendState::Registered {
        interest: Interest::READABLE,
        mode: Mode::OneShot,
        arm: ArmState::Armed,
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
