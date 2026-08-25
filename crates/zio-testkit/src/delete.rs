//! Delete-branch conformance scenarios.

use zio::{
    ArmState, Error, Interest, Key, Mode, Registration, RegistrationState,
    test_support::{MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll},
};

use crate::{
    Branch, ConformanceCheck, ConformanceFailure, MutationScenario,
    calls::{
        expect_delete_call, expect_disarm_call, expect_exact_register_call, expect_register_call,
    },
    delete_failure::{expect_failed_delete, unexpected_delete},
    verify::{
        PRIOR_INTEREST, PRIOR_MODE, backend_registered, expect_backend, expect_duplicate,
        expect_stale, expect_state, finish, mismatch, outcome, registered, source,
    },
};

const KEY: Key = Key::new(303);

pub(crate) fn run(scenario: MutationScenario) -> Result<(), ConformanceFailure> {
    let mut steps = vec![
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Delete(outcome(scenario)),
    ];
    match scenario.branch() {
        Branch::Success | Branch::Applied => {
            steps.push(MutationStep::Register(MutationOutcome::Success));
            steps.push(MutationStep::Delete(MutationOutcome::Success));
        }
        Branch::NotApplied | Branch::Unknown => {
            steps.push(MutationStep::Delete(MutationOutcome::Success));
        }
    }
    let mut poll = ScriptedPoll::with_capacity(1, steps).map_err(|error| {
        ConformanceFailure::new(
            scenario,
            ConformanceCheck::Setup,
            "scripted poll",
            error.to_string(),
        )
    })?;
    let source = source(scenario)?;
    let registration = poll
        .register(&source, KEY, PRIOR_INTEREST, PRIOR_MODE)
        .map_err(|error| {
            ConformanceFailure::new(
                scenario,
                ConformanceCheck::Setup,
                "initial registration",
                error.to_string(),
            )
        })?;
    let id = registration.id();
    expect_exact_register_call(&poll, 0, id, KEY, PRIOR_INTEREST, PRIOR_MODE, scenario)?;
    poll.establish_disarmed(&registration).map_err(|error| {
        ConformanceFailure::new(
            scenario,
            ConformanceCheck::Setup,
            "disarmed one-shot registration",
            error.to_string(),
        )
    })?;
    expect_disarm_call(&poll, 1, id, scenario)?;
    let result = poll.delete(registration);
    expect_delete_call(
        &poll,
        2,
        id,
        PRIOR_INTEREST,
        registered(ArmState::Disarmed),
        scenario,
    )?;

    match scenario.branch() {
        Branch::Success => {
            result.map_err(|error| unexpected_delete(scenario, &error))?;
            expect_backend(&poll, id, ScriptedBackendState::Absent, scenario)?;
            replace_and_cleanup(&mut poll, &source, id, scenario)?;
        }
        Branch::NotApplied => {
            let registration = expect_failed_delete(result, id, scenario)?;
            expect_state(
                &poll,
                &registration,
                registered(ArmState::Disarmed),
                scenario,
            )?;
            expect_backend(
                &poll,
                id,
                backend_registered(PRIOR_INTEREST, PRIOR_MODE, ArmState::Disarmed),
                scenario,
            )?;
            expect_duplicate(&mut poll, &source, id, scenario)?;
            retry_delete(
                &mut poll,
                registration,
                registered(ArmState::Disarmed),
                scenario,
            )?;
        }
        Branch::Applied => {
            let registration = expect_failed_delete(result, id, scenario)?;
            expect_stale(&poll, &registration, scenario)?;
            expect_backend(&poll, id, ScriptedBackendState::Absent, scenario)?;
            replace_and_cleanup(&mut poll, &source, id, scenario)?;
        }
        Branch::Unknown => {
            let registration = expect_failed_delete(result, id, scenario)?;
            expect_state(
                &poll,
                &registration,
                zio::RegistrationState::Uncertain,
                scenario,
            )?;
            expect_backend(&poll, id, ScriptedBackendState::Unknown, scenario)?;
            expect_duplicate(&mut poll, &source, id, scenario)?;
            let calls = poll.calls().len();
            match poll.modify(&registration, Interest::WRITABLE, Mode::Level) {
                Err(Error::Uncertain {
                    registration: actual,
                }) if actual == id => {}
                actual => {
                    return mismatch(
                        scenario,
                        ConformanceCheck::State,
                        format!("Uncertain({id:?})"),
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
            retry_delete(
                &mut poll,
                registration,
                RegistrationState::Uncertain,
                scenario,
            )?;
        }
    }
    finish(&poll, scenario)
}

fn retry_delete(
    poll: &mut ScriptedPoll,
    registration: Registration,
    state: RegistrationState,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    let id = registration.id();
    let call_index = poll.calls().len();
    poll.delete(registration).map_err(|error| {
        ConformanceFailure::new(
            scenario,
            ConformanceCheck::Retry,
            format!("successful deletion retry of {id:?}"),
            error.to_string(),
        )
    })?;
    expect_delete_call(poll, call_index, id, PRIOR_INTEREST, state, scenario)?;
    expect_backend(poll, id, ScriptedBackendState::Absent, scenario)
}

fn replace_and_cleanup(
    poll: &mut ScriptedPoll,
    source: &std::os::unix::net::UnixStream,
    retired: zio::RegistrationId,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    let call_index = poll.calls().len();
    let replacement = poll
        .register(source, KEY, PRIOR_INTEREST, PRIOR_MODE)
        .map_err(|error| {
            ConformanceFailure::new(
                scenario,
                ConformanceCheck::DuplicateRetention,
                "descriptor released after applied deletion",
                error.to_string(),
            )
        })?;
    let observed =
        expect_register_call(poll, call_index, KEY, PRIOR_INTEREST, PRIOR_MODE, scenario)?;
    if observed != replacement.id() {
        return mismatch(
            scenario,
            ConformanceCheck::Handle,
            replacement.id(),
            observed,
        );
    }
    if replacement.id() == retired {
        return mismatch(
            scenario,
            ConformanceCheck::Handle,
            "fresh registration generation",
            replacement.id(),
        );
    }
    retry_delete(poll, replacement, registered(ArmState::Armed), scenario)
}
