//! Register-branch conformance scenarios.

use zio::{
    ArmState, Error, Interest, Key, Mode, Registration, RegistrationState,
    test_support::{MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll},
};

use crate::{
    Branch, ConformanceCheck, ConformanceFailure, MutationScenario,
    calls::{expect_delete_call, expect_register_call},
    register_failure::{expect_failed_register, expect_handle_id},
    verify::{
        backend_registered, expect_backend, expect_mutation, expect_retained_capacity,
        expect_state, finish, mismatch, outcome, registered, source,
    },
};

const KEY: Key = Key::new(101);
const INTEREST: Interest = Interest::READABLE;
const MODE: Mode = Mode::OneShot;

pub(crate) fn run(scenario: MutationScenario) -> Result<(), ConformanceFailure> {
    let mut steps = vec![MutationStep::Register(outcome(scenario))];
    match scenario.branch() {
        Branch::NotApplied => steps.push(MutationStep::Register(MutationOutcome::Success)),
        Branch::Success | Branch::Applied | Branch::Unknown => {}
    }
    steps.push(MutationStep::Delete(MutationOutcome::Success));
    let mut poll = ScriptedPoll::with_capacity(1, steps).map_err(|error| {
        ConformanceFailure::new(
            scenario,
            ConformanceCheck::Setup,
            "scripted poll",
            error.to_string(),
        )
    })?;
    let source = source(scenario)?;
    let result = poll.register(&source, KEY, INTEREST, MODE);
    let observed = expect_register_call(&poll, 0, KEY, INTEREST, MODE, scenario)?;
    match scenario.branch() {
        Branch::Success => {
            let registration = result.map_err(|error| unexpected_register(scenario, &error))?;
            expect_handle_id(&registration, observed, scenario)?;
            validate_retained(&mut poll, &source, &registration, scenario, false)?;
            cleanup(
                &mut poll,
                registration,
                registered(ArmState::Armed),
                scenario,
            )?;
        }
        Branch::NotApplied => {
            let (error, _registration) = expect_failed_register(result, None, scenario)?;
            expect_mutation(scenario, error)?;
            expect_backend(&poll, observed, ScriptedBackendState::Absent, scenario)?;
            let retry = poll
                .register(&source, KEY, INTEREST, MODE)
                .map_err(|error| retry_register(scenario, &error))?;
            let retried = expect_register_call(&poll, 1, KEY, INTEREST, MODE, scenario)?;
            expect_handle_id(&retry, retried, scenario)?;
            if retry.id() == observed {
                return mismatch(
                    scenario,
                    ConformanceCheck::Retry,
                    "fresh registration generation",
                    retry.id(),
                );
            }
            validate_retained(&mut poll, &source, &retry, scenario, false)?;
            cleanup(&mut poll, retry, registered(ArmState::Armed), scenario)?;
        }
        Branch::Applied => {
            let (error, registration) = expect_failed_register(result, Some(observed), scenario)?;
            let registration = registration.ok_or_else(|| {
                ConformanceFailure::new(
                    scenario,
                    ConformanceCheck::Handle,
                    "retained applied registration",
                    "no registration",
                )
            })?;
            expect_mutation(scenario, error)?;
            validate_retained(&mut poll, &source, &registration, scenario, false)?;
            cleanup(
                &mut poll,
                registration,
                registered(ArmState::Armed),
                scenario,
            )?;
        }
        Branch::Unknown => {
            let (error, registration) = expect_failed_register(result, Some(observed), scenario)?;
            let registration = registration.ok_or_else(|| {
                ConformanceFailure::new(
                    scenario,
                    ConformanceCheck::Handle,
                    "retained uncertain registration",
                    "no registration",
                )
            })?;
            expect_mutation(scenario, error)?;
            validate_retained(&mut poll, &source, &registration, scenario, true)?;
            let calls = poll.calls().len();
            match poll.modify(&registration, Interest::WRITABLE, Mode::Level) {
                Err(Error::Uncertain {
                    registration: actual,
                }) if actual == registration.id() => {}
                actual => {
                    return mismatch(
                        scenario,
                        ConformanceCheck::State,
                        format!("Uncertain({:?})", registration.id()),
                        format!("{actual:?}"),
                    );
                }
            }
            if poll.calls().len() != calls {
                return mismatch(
                    scenario,
                    ConformanceCheck::Script,
                    "uncertain modify rejected before backend",
                    "backend call",
                );
            }
            cleanup(
                &mut poll,
                registration,
                RegistrationState::Uncertain,
                scenario,
            )?;
        }
    }
    finish(&poll, scenario)
}

fn validate_retained(
    poll: &mut ScriptedPoll,
    source: &std::os::unix::net::UnixStream,
    registration: &Registration,
    scenario: MutationScenario,
    uncertain: bool,
) -> Result<(), ConformanceFailure> {
    if uncertain {
        expect_state(
            poll,
            registration,
            zio::RegistrationState::Uncertain,
            scenario,
        )?;
        expect_backend(
            poll,
            registration.id(),
            ScriptedBackendState::Unknown,
            scenario,
        )?;
    } else {
        expect_state(poll, registration, registered(ArmState::Armed), scenario)?;
        expect_backend(
            poll,
            registration.id(),
            backend_registered(INTEREST, MODE, ArmState::Armed),
            scenario,
        )?;
    }
    expect_retained_capacity(poll, source, scenario)
}

fn cleanup(
    poll: &mut ScriptedPoll,
    registration: Registration,
    expected_state: RegistrationState,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    let id = registration.id();
    let call_index = poll.calls().len();
    poll.delete(registration).map_err(|error| {
        ConformanceFailure::new(
            scenario,
            ConformanceCheck::Retry,
            format!("successful cleanup of {id:?}"),
            error.to_string(),
        )
    })?;
    expect_delete_call(poll, call_index, id, INTEREST, expected_state, scenario)?;
    expect_backend(poll, id, ScriptedBackendState::Absent, scenario)
}

fn unexpected_register(
    scenario: MutationScenario,
    error: &zio::RegisterError,
) -> ConformanceFailure {
    ConformanceFailure::new(
        scenario,
        ConformanceCheck::Result,
        "successful registration",
        error.to_string(),
    )
}

fn retry_register(scenario: MutationScenario, error: &zio::RegisterError) -> ConformanceFailure {
    ConformanceFailure::new(
        scenario,
        ConformanceCheck::Retry,
        "successful registration retry",
        error.to_string(),
    )
}
