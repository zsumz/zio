//! Modify-branch conformance scenarios.

use zio::{
    ArmState, Error, Interest, Key, Registration, RegistrationState,
    test_support::{MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll},
};

use crate::{
    Branch, ConformanceCheck, ConformanceFailure, MutationScenario,
    calls::{
        expect_delete_call, expect_disarm_call, expect_exact_register_call, expect_modify_call,
    },
    modify_commit::confirm_committed_prior,
    verify::{
        DESIRED_INTEREST, DESIRED_MODE, PRIOR_INTEREST, PRIOR_MODE, backend_registered,
        expect_backend, expect_mutation, expect_retained_capacity, expect_state, finish, mismatch,
        outcome, registered, source,
    },
};

const KEY: Key = Key::new(202);

pub(crate) fn run(scenario: MutationScenario) -> Result<(), ConformanceFailure> {
    let mut steps = vec![
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Modify(outcome(scenario)),
    ];
    if scenario.branch() == Branch::NotApplied {
        steps.push(MutationStep::Modify(MutationOutcome::Success));
    }
    if scenario.branch() != Branch::Unknown {
        steps.push(MutationStep::Modify(MutationOutcome::Success));
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

    let result = poll.modify(&registration, DESIRED_INTEREST, DESIRED_MODE);
    expect_modify_payload(&poll, 2, id, scenario)?;
    match scenario.branch() {
        Branch::Success => {
            result.map_err(|error| unexpected_modify(scenario, &error))?;
            expect_desired(&poll, &registration, scenario)?;
        }
        Branch::NotApplied => {
            let error = expect_failed(result, scenario)?;
            expect_mutation(scenario, error)?;
            expect_prior(&poll, &registration, scenario)?;
            poll.modify(&registration, DESIRED_INTEREST, DESIRED_MODE)
                .map_err(|error| {
                    ConformanceFailure::new(
                        scenario,
                        ConformanceCheck::Retry,
                        "successful modification retry",
                        error.to_string(),
                    )
                })?;
            expect_modify_payload(&poll, 3, id, scenario)?;
            expect_desired(&poll, &registration, scenario)?;
        }
        Branch::Applied => {
            let error = expect_failed(result, scenario)?;
            expect_mutation(scenario, error)?;
            expect_desired(&poll, &registration, scenario)?;
        }
        Branch::Unknown => {
            let error = expect_failed(result, scenario)?;
            expect_mutation(scenario, error)?;
            expect_state(
                &poll,
                &registration,
                zio::RegistrationState::Uncertain,
                scenario,
            )?;
            expect_backend(&poll, id, ScriptedBackendState::Unknown, scenario)?;
            let calls = poll.calls().len();
            match poll.modify(&registration, PRIOR_INTEREST, PRIOR_MODE) {
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
        }
    }
    if scenario.branch() != Branch::Unknown {
        confirm_committed_prior(&mut poll, &registration, scenario)?;
    }
    expect_retained_capacity(&mut poll, &source, scenario)?;
    let (interest, state) = if scenario.branch() == Branch::Unknown {
        (PRIOR_INTEREST, RegistrationState::Uncertain)
    } else {
        (DESIRED_INTEREST, registered(ArmState::Armed))
    };
    cleanup(&mut poll, registration, interest, state, scenario)?;
    finish(&poll, scenario)
}

fn expect_modify_payload(
    poll: &ScriptedPoll,
    index: usize,
    registration: zio::RegistrationId,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    expect_modify_call(
        poll,
        index,
        registration,
        (PRIOR_INTEREST, PRIOR_MODE, ArmState::Disarmed),
        (DESIRED_INTEREST, DESIRED_MODE),
        scenario,
    )
}

fn expect_failed(
    result: Result<(), Error>,
    scenario: MutationScenario,
) -> Result<Error, ConformanceFailure> {
    match result {
        Ok(()) => mismatch(
            scenario,
            ConformanceCheck::Result,
            "mutation failure",
            "success",
        ),
        Err(error) => Ok(error),
    }
}

fn expect_prior(
    poll: &ScriptedPoll,
    registration: &Registration,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    expect_state(poll, registration, registered(ArmState::Disarmed), scenario)?;
    expect_backend(
        poll,
        registration.id(),
        backend_registered(PRIOR_INTEREST, PRIOR_MODE, ArmState::Disarmed),
        scenario,
    )
}

fn expect_desired(
    poll: &ScriptedPoll,
    registration: &Registration,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    expect_state(poll, registration, registered(ArmState::Armed), scenario)?;
    expect_backend(
        poll,
        registration.id(),
        backend_registered(DESIRED_INTEREST, DESIRED_MODE, ArmState::Armed),
        scenario,
    )
}

fn cleanup(
    poll: &mut ScriptedPoll,
    registration: Registration,
    interest: Interest,
    state: RegistrationState,
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
    expect_delete_call(poll, call_index, id, interest, state, scenario)?;
    expect_backend(poll, id, ScriptedBackendState::Absent, scenario)
}

fn unexpected_modify(scenario: MutationScenario, error: &Error) -> ConformanceFailure {
    ConformanceFailure::new(
        scenario,
        ConformanceCheck::Result,
        "successful modification",
        error.to_string(),
    )
}
