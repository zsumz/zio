//! Stale and wrong-poller probes that must avoid backend work.

use zio::{
    ArmState, Error, Interest, Mode, Registration, RegistrationId, RegistrationState,
    test_support::ScriptedBackendState,
};

use crate::{
    ModelSequenceCheck,
    model_sequence_failure::Divergence,
    model_sequence_runner::{STRANGER_INTEREST, STRANGER_MODE, SequenceContext},
};

pub(crate) fn probe_stale(context: &mut SequenceContext) -> Result<(), Divergence> {
    let registration = context
        .model
        .stale()
        .ok_or_else(|| precondition("retired handle", "no retired handle"))?;
    let calls = context.poll.calls().len();
    expect_stale_state(
        context.poll.registration_state(&registration),
        registration.id(),
    )?;
    expect_stale(
        context
            .poll
            .modify(&registration, Interest::READABLE, Mode::OneShot),
        registration.id(),
    )?;
    let Err(error) = context.poll.delete(registration) else {
        return Err(result("stale delete rejection", "success"));
    };
    let (cause, returned) = error.into_parts();
    if returned != registration {
        return Err(handle(registration, returned));
    }
    expect_stale(Err(cause), registration.id())?;
    unchanged_calls(calls, context.poll.calls().len(), "stale probe")
}

pub(crate) fn probe_wrong_poller(context: &mut SequenceContext) -> Result<(), Divergence> {
    let registration = context
        .model
        .active()
        .map(|entry| entry.registration)
        .ok_or_else(|| precondition("active handle", "vacant slot"))?;
    let calls = context.stranger.calls().len();
    expect_wrong_state(
        context.stranger.registration_state(&registration),
        registration.id(),
    )?;
    expect_wrong(
        context
            .stranger
            .modify(&registration, Interest::WRITABLE, Mode::Level),
        registration.id(),
    )?;
    let Err(error) = context.stranger.delete(registration) else {
        return Err(result("wrong-poller delete rejection", "success"));
    };
    let (cause, returned) = error.into_parts();
    if returned != registration {
        return Err(handle(registration, returned));
    }
    expect_wrong(Err(cause), registration.id())?;
    unchanged_calls(calls, context.stranger.calls().len(), "wrong-poller probe")
}

fn expect_stale_state(
    result_value: Result<RegistrationState, Error>,
    expected: RegistrationId,
) -> Result<(), Divergence> {
    match result_value {
        Err(error) => expect_stale(Err(error), expected),
        Ok(actual) => Err(result("Stale state error", actual)),
    }
}

fn expect_stale(
    result_value: Result<(), Error>,
    expected: RegistrationId,
) -> Result<(), Divergence> {
    match result_value {
        Err(Error::Stale { registration }) if registration == expected => Ok(()),
        actual => Err(result(
            format!("Stale({expected:?})"),
            format!("{actual:?}"),
        )),
    }
}

fn expect_wrong_state(
    result_value: Result<RegistrationState, Error>,
    expected: RegistrationId,
) -> Result<(), Divergence> {
    match result_value {
        Err(error) => expect_wrong(Err(error), expected),
        Ok(actual) => Err(result("WrongPoller state error", actual)),
    }
}

fn expect_wrong(
    result_value: Result<(), Error>,
    expected: RegistrationId,
) -> Result<(), Divergence> {
    match result_value {
        Err(Error::WrongPoller { registration }) if registration == expected => Ok(()),
        actual => Err(result(
            format!("WrongPoller({expected:?})"),
            format!("{actual:?}"),
        )),
    }
}

pub(crate) fn verify_stranger(context: &SequenceContext) -> Result<(), Divergence> {
    let state = context
        .stranger
        .registration_state(&context.stranger_registration)
        .map_err(|error| result("registered stranger handle", error))?;
    if state
        != (RegistrationState::Registered {
            arm: ArmState::Armed,
        })
    {
        return Err(Divergence::new(
            ModelSequenceCheck::State,
            format!(
                "{:?}",
                RegistrationState::Registered {
                    arm: ArmState::Armed
                }
            ),
            format!("{state:?}"),
        ));
    }
    let expected_backend = ScriptedBackendState::Registered {
        interest: STRANGER_INTEREST,
        mode: STRANGER_MODE,
        arm: ArmState::Armed,
    };
    let actual_backend = context
        .stranger
        .backend_state(context.stranger_registration.id());
    if actual_backend != expected_backend {
        return Err(Divergence::new(
            ModelSequenceCheck::Backend,
            format!("{expected_backend:?}"),
            format!("{actual_backend:?}"),
        ));
    }
    if context.stranger.calls().len() == 1 {
        Ok(())
    } else {
        Err(Divergence::new(
            ModelSequenceCheck::Calls,
            "one stranger setup call",
            format!("{} calls", context.stranger.calls().len()),
        ))
    }
}

fn unchanged_calls(expected: usize, actual: usize, context: &str) -> Result<(), Divergence> {
    if expected == actual {
        Ok(())
    } else {
        Err(Divergence::new(
            ModelSequenceCheck::Calls,
            format!("{expected} calls before {context}"),
            format!("{actual} calls after {context}"),
        ))
    }
}

fn precondition(expected: impl std::fmt::Debug, actual: impl std::fmt::Debug) -> Divergence {
    Divergence::new(
        ModelSequenceCheck::Precondition,
        format!("{expected:?}"),
        format!("{actual:?}"),
    )
}

fn result(expected: impl std::fmt::Debug, actual: impl std::fmt::Debug) -> Divergence {
    Divergence::new(
        ModelSequenceCheck::Result,
        format!("{expected:?}"),
        format!("{actual:?}"),
    )
}

fn handle(expected: Registration, actual: Registration) -> Divergence {
    Divergence::new(
        ModelSequenceCheck::Handle,
        format!("{expected:?}"),
        format!("{actual:?}"),
    )
}
