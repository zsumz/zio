//! Feed-forward verification for authoritative modifications.

use zio::{ArmState, Registration, test_support::ScriptedPoll};

use crate::{
    ConformanceCheck, ConformanceFailure, MutationScenario,
    calls::expect_modify_call,
    setup::{DESIRED_INTEREST, DESIRED_MODE},
};

pub(crate) fn confirm_committed_prior(
    poll: &mut ScriptedPoll,
    registration: &Registration,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    let call_index = poll.calls().len();
    poll.modify(registration, DESIRED_INTEREST, DESIRED_MODE)
        .map_err(|error| {
            ConformanceFailure::new(
                scenario,
                ConformanceCheck::Retry,
                "successful same-desired confirmation",
                error.to_string(),
            )
        })?;
    expect_modify_call(
        poll,
        call_index,
        registration.id(),
        (DESIRED_INTEREST, DESIRED_MODE, ArmState::Armed),
        (DESIRED_INTEREST, DESIRED_MODE),
        scenario,
    )
}
