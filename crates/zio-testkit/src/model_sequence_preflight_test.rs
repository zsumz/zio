//! Invalid-interest preflight controls for slots and retained state.

use std::{io, os::unix::net::UnixStream};

use zio::{
    ArmState, Error, Interest, Key, Mode, RegistrationState,
    test_support::{MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll},
};

#[test]
fn invalid_register_does_not_consume_a_generation() -> Result<(), io::Error> {
    let steps = [MutationStep::Register(MutationOutcome::Success)];
    let mut subject = scripted(steps)?;
    let mut control = scripted(steps)?;
    let subject_source = source()?;
    let control_source = source()?;
    let key = Key::new(0x5a10_0000_0000_0001);

    let Err(error) = subject.register(&subject_source, key, Interest::EMPTY, Mode::Level) else {
        return Err(io::Error::other(
            "invalid registration unexpectedly succeeded",
        ));
    };
    if error.registration().is_some() {
        return Err(io::Error::other(
            "invalid registration returned a registration capability",
        ));
    }
    let (cause, returned) = error.into_parts();
    require_invalid_interest(&cause)?;
    check_eq(&returned, &None, "invalid registration return")?;
    check_eq(&subject.calls().len(), &0, "invalid registration calls")?;

    let subject_registration = subject
        .register(&subject_source, key, Interest::READABLE, Mode::Level)
        .map_err(other)?;
    let control_registration = control
        .register(&control_source, key, Interest::READABLE, Mode::Level)
        .map_err(other)?;
    check_eq(
        &subject_registration.id(),
        &control_registration.id(),
        "first valid generation after invalid preflight",
    )?;
    check_eq(subject.calls(), control.calls(), "valid registration call")?;
    subject.finish().map_err(other)?;
    control.finish().map_err(other)
}

#[test]
fn invalid_modify_preserves_the_exact_prior_tuple() -> Result<(), io::Error> {
    let steps = [
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Modify(MutationOutcome::Success),
    ];
    let mut poll = scripted(steps)?;
    let source = source()?;
    let registration = poll
        .register(
            &source,
            Key::new(0x5a10_0000_0000_0002),
            Interest::READABLE,
            Mode::OneShot,
        )
        .map_err(other)?;
    poll.establish_disarmed(&registration).map_err(other)?;

    let prior_state = poll.registration_state(&registration).map_err(other)?;
    let prior_backend = poll.backend_state(registration.id());
    let prior_calls = poll.calls().to_vec();
    check_eq(
        &prior_state,
        &RegistrationState::Registered {
            arm: ArmState::Disarmed,
        },
        "disarmed portable state",
    )?;
    check_eq(
        &prior_backend,
        &ScriptedBackendState::Registered {
            interest: Interest::READABLE,
            mode: Mode::OneShot,
            arm: ArmState::Disarmed,
        },
        "disarmed backend tuple",
    )?;

    let Err(error) = poll.modify(&registration, Interest::EMPTY, Mode::Level) else {
        return Err(io::Error::other(
            "invalid modification unexpectedly succeeded",
        ));
    };
    require_invalid_interest(&error)?;
    check_eq(
        &poll.registration_state(&registration).map_err(other)?,
        &prior_state,
        "portable state after invalid modification",
    )?;
    check_eq(
        &poll.backend_state(registration.id()),
        &prior_backend,
        "backend tuple after invalid modification",
    )?;
    check_eq(
        poll.calls(),
        prior_calls.as_slice(),
        "calls after invalid modification",
    )?;

    poll.modify(&registration, Interest::WRITABLE, Mode::Level)
        .map_err(other)?;
    poll.finish().map_err(other)
}

fn scripted(steps: impl IntoIterator<Item = MutationStep>) -> Result<ScriptedPoll, io::Error> {
    ScriptedPoll::with_capacity(1, steps).map_err(other)
}

fn source() -> Result<UnixStream, io::Error> {
    UnixStream::pair().map(|pair| pair.0)
}

fn require_invalid_interest(error: &Error) -> Result<(), io::Error> {
    if matches!(error, Error::InvalidInterest) {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "expected InvalidInterest, observed {error:?}"
        )))
    }
}

fn other(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

fn check_eq<T>(actual: &T, expected: &T, context: &str) -> Result<(), io::Error>
where
    T: std::fmt::Debug + PartialEq + ?Sized,
{
    if actual == expected {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{context}: expected {expected:?}, observed {actual:?}"
        )))
    }
}
