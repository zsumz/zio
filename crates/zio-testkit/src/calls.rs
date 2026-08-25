//! Exact normalized backend-call checks.

use zio::{
    ArmState, Interest, Key, Mode, RegistrationId, RegistrationState,
    test_support::{MutationCall, ScriptedPoll},
};

use crate::{ConformanceCheck, ConformanceFailure, MutationScenario, verify::mismatch};

pub(crate) fn expect_call(
    poll: &ScriptedPoll,
    index: usize,
    expected: MutationCall,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    match poll.calls().get(index).copied() {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => mismatch(scenario, ConformanceCheck::Script, expected, actual),
        None => mismatch(
            scenario,
            ConformanceCheck::Script,
            expected,
            format!("no call at index {index}"),
        ),
    }
}

pub(crate) fn expect_exact_register_call(
    poll: &ScriptedPoll,
    index: usize,
    registration: RegistrationId,
    key: Key,
    interest: Interest,
    mode: Mode,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    expect_call(
        poll,
        index,
        MutationCall::Register {
            registration,
            key,
            interest,
            mode,
        },
        scenario,
    )
}

pub(crate) fn expect_modify_call(
    poll: &ScriptedPoll,
    index: usize,
    registration: RegistrationId,
    previous: (Interest, Mode, ArmState),
    desired: (Interest, Mode),
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    expect_call(
        poll,
        index,
        MutationCall::Modify {
            registration,
            previous_interest: previous.0,
            previous_mode: previous.1,
            previous_arm: previous.2,
            desired_interest: desired.0,
            desired_mode: desired.1,
        },
        scenario,
    )
}

pub(crate) fn expect_delete_call(
    poll: &ScriptedPoll,
    index: usize,
    registration: RegistrationId,
    interest: Interest,
    state: RegistrationState,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    expect_call(
        poll,
        index,
        MutationCall::Delete {
            registration,
            interest,
            state,
        },
        scenario,
    )
}

pub(crate) fn expect_disarm_call(
    poll: &ScriptedPoll,
    index: usize,
    registration: RegistrationId,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    expect_call(
        poll,
        index,
        MutationCall::EstablishDisarmed { registration },
        scenario,
    )
}

pub(crate) fn expect_register_call(
    poll: &ScriptedPoll,
    index: usize,
    key: Key,
    interest: Interest,
    mode: Mode,
    scenario: MutationScenario,
) -> Result<RegistrationId, ConformanceFailure> {
    match poll.calls().get(index).copied() {
        Some(MutationCall::Register {
            registration,
            key: actual_key,
            interest: actual_interest,
            mode: actual_mode,
        }) if (actual_key, actual_interest, actual_mode) == (key, interest, mode) => {
            Ok(registration)
        }
        Some(actual) => mismatch(
            scenario,
            ConformanceCheck::Script,
            format!("Register {{ key: {key:?}, interest: {interest:?}, mode: {mode:?} }}"),
            actual,
        ),
        None => mismatch(
            scenario,
            ConformanceCheck::Script,
            "register call",
            format!("no call at index {index}"),
        ),
    }
}
