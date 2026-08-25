//! Copyable registration behavior across every delete outcome.

use std::{error::Error as StdError, fmt::Debug, io, os::unix::net::UnixStream};

use zio::{
    ArmState, CommitStatus, DeleteError, Error, Interest, Key, Mode, Operation, Registration,
    RegistrationId, RegistrationState,
};
use zio_testkit::support::{MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll};

const KEY: Key = Key::new(801);
const INTEREST: Interest = Interest::READABLE;
const MODE: Mode = Mode::OneShot;

#[test]
fn delete_outcome_branches_update_every_surviving_copy() -> Result<(), Box<dyn StdError>> {
    verify_success()?;
    verify_not_applied()?;
    verify_applied()?;
    verify_unknown()?;
    Ok(())
}

fn verify_success() -> Result<(), Box<dyn StdError>> {
    let (mut poll, registration) = registered_poll([
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Delete(MutationOutcome::Success),
    ])?;
    let survivor = registration;
    let id = registration.id();

    poll.delete(registration)?;
    check_eq(
        &poll.backend_state(id),
        &ScriptedBackendState::Absent,
        "successful delete backend state",
    )?;
    expect_stale_without_backend_calls(&mut poll, survivor)?;
    poll.finish()?;
    Ok(())
}

fn verify_not_applied() -> Result<(), Box<dyn StdError>> {
    let (mut poll, registration) = registered_poll([
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Delete(failure(CommitStatus::NotApplied)),
        MutationStep::Delete(MutationOutcome::Success),
    ])?;
    let survivor = registration;
    let id = registration.id();
    let returned = failed_delete(poll.delete(registration), CommitStatus::NotApplied, id)?;

    expect_state(&poll, survivor, armed())?;
    expect_state(&poll, returned, armed())?;
    check_eq(
        &poll.backend_state(id),
        &backend_registered(),
        "not-applied backend state",
    )?;
    poll.delete(returned)?;
    expect_stale_without_backend_calls(&mut poll, survivor)?;
    poll.finish()?;
    Ok(())
}

fn verify_applied() -> Result<(), Box<dyn StdError>> {
    let (mut poll, registration) = registered_poll([
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Delete(failure(CommitStatus::Applied)),
    ])?;
    let survivor = registration;
    let id = registration.id();
    let returned = failed_delete(poll.delete(registration), CommitStatus::Applied, id)?;

    check_eq(
        &poll.backend_state(id),
        &ScriptedBackendState::Absent,
        "applied backend state",
    )?;
    expect_stale_without_backend_calls(&mut poll, survivor)?;
    expect_stale_without_backend_calls(&mut poll, returned)?;
    poll.finish()?;
    Ok(())
}

fn verify_unknown() -> Result<(), Box<dyn StdError>> {
    let (mut poll, registration) = registered_poll([
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Delete(failure(CommitStatus::Unknown)),
        MutationStep::Delete(MutationOutcome::Success),
    ])?;
    let survivor = registration;
    let id = registration.id();
    let returned = failed_delete(poll.delete(registration), CommitStatus::Unknown, id)?;

    expect_state(&poll, survivor, RegistrationState::Uncertain)?;
    expect_state(&poll, returned, RegistrationState::Uncertain)?;
    check_eq(
        &poll.backend_state(id),
        &ScriptedBackendState::Unknown,
        "unknown backend state",
    )?;
    let calls = poll.calls().len();
    expect_uncertain(poll.modify(&survivor, Interest::WRITABLE, Mode::Level), id)?;
    check_eq(&poll.calls().len(), &calls, "uncertain modify call count")?;
    poll.delete(returned)?;
    expect_stale_without_backend_calls(&mut poll, survivor)?;
    poll.finish()?;
    Ok(())
}

fn registered_poll(
    steps: impl IntoIterator<Item = MutationStep>,
) -> Result<(ScriptedPoll, Registration), Box<dyn StdError>> {
    let source = UnixStream::pair()?.0;
    let mut poll = ScriptedPoll::with_capacity(1, steps)?;
    let registration = poll.register(&source, KEY, INTEREST, MODE)?;
    Ok((poll, registration))
}

fn failed_delete(
    result: Result<(), DeleteError>,
    expected_commit: CommitStatus,
    expected_id: RegistrationId,
) -> Result<Registration, Box<dyn StdError>> {
    let Err(error) = result else {
        return Err(io::Error::other("deletion unexpectedly succeeded").into());
    };
    let borrowed = *error.registration();
    check_eq(&borrowed.id(), &expected_id, "borrowed error handle")?;
    let (cause, returned) = error.into_parts();
    check_eq(&returned.id(), &expected_id, "returned error handle")?;
    match cause {
        Error::Mutation(mutation) => {
            check_eq(
                &mutation.operation(),
                &Operation::Delete,
                "delete operation",
            )?;
            check_eq(&mutation.commit(), &expected_commit, "delete commit")?;
        }
        actual => return Err(failure_message("Mutation error", actual).into()),
    }
    check_eq(&borrowed, &returned, "error handle copies")?;
    Ok(returned)
}

fn expect_stale_without_backend_calls(
    poll: &mut ScriptedPoll,
    registration: Registration,
) -> Result<(), Box<dyn StdError>> {
    let id = registration.id();
    let calls = poll.calls().len();
    expect_stale_state(poll.registration_state(&registration), id)?;
    expect_stale(
        poll.modify(&registration, Interest::WRITABLE, Mode::Level),
        id,
    )?;
    let Err(error) = poll.delete(registration) else {
        return Err(io::Error::other("stale deletion succeeded").into());
    };
    let (cause, returned) = error.into_parts();
    expect_stale(Err(cause), id)?;
    check_eq(&returned.id(), &id, "stale returned handle")?;
    check_eq(&poll.calls().len(), &calls, "stale backend call count")?;
    Ok(())
}

fn expect_state(
    poll: &ScriptedPoll,
    registration: Registration,
    expected: RegistrationState,
) -> Result<(), Box<dyn StdError>> {
    check_eq(
        &poll.registration_state(&registration)?,
        &expected,
        "registration copy state",
    )?;
    Ok(())
}

fn expect_stale_state(
    result: Result<RegistrationState, Error>,
    expected: RegistrationId,
) -> Result<(), io::Error> {
    match result {
        Err(error) => expect_stale(Err(error), expected),
        Ok(actual) => Err(failure_message("Stale state", actual)),
    }
}

fn expect_stale(result: Result<(), Error>, expected: RegistrationId) -> Result<(), io::Error> {
    match result {
        Err(Error::Stale { registration }) => check_eq(&registration, &expected, "stale identity"),
        actual => Err(failure_message("Stale error", actual)),
    }
}

fn expect_uncertain(result: Result<(), Error>, expected: RegistrationId) -> Result<(), io::Error> {
    match result {
        Err(Error::Uncertain { registration }) => {
            check_eq(&registration, &expected, "uncertain identity")
        }
        actual => Err(failure_message("Uncertain error", actual)),
    }
}

const fn failure(commit: CommitStatus) -> MutationOutcome {
    MutationOutcome::Failure {
        commit,
        kind: io::ErrorKind::BrokenPipe,
    }
}

const fn armed() -> RegistrationState {
    RegistrationState::Registered {
        arm: ArmState::Armed,
    }
}

const fn backend_registered() -> ScriptedBackendState {
    ScriptedBackendState::Registered {
        interest: INTEREST,
        mode: MODE,
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

fn failure_message(expected: impl Debug, actual: impl Debug) -> io::Error {
    io::Error::other(format!("expected {expected:?}, observed {actual:?}"))
}
