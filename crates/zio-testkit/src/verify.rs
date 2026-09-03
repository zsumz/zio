//! Shared scenario construction and contract checks.

use std::{fmt::Debug, os::unix::net::UnixStream};

use zio::{
    CapacityKind, CapacityReason, Error, Interest, Mode, Registration, RegistrationId,
    RegistrationState,
    test_support::{ScriptedBackendState, ScriptedPoll},
};

use crate::{ConformanceCheck, ConformanceFailure, MutationScenario};

pub(crate) use crate::setup::{
    DESIRED_INTEREST, DESIRED_MODE, PRIOR_INTEREST, PRIOR_MODE, backend_registered, outcome,
    registered, source, source_kind,
};

pub(crate) fn expect_mutation(
    scenario: MutationScenario,
    error: Error,
) -> Result<(), ConformanceFailure> {
    let Error::Mutation(mutation) = error else {
        return mismatch(
            scenario,
            ConformanceCheck::Result,
            "Error::Mutation",
            format!("{error:?}"),
        );
    };
    if mutation.operation() != scenario.operation().operation() {
        return mismatch(
            scenario,
            ConformanceCheck::Operation,
            scenario.operation().operation(),
            mutation.operation(),
        );
    }
    let expected_commit = scenario.branch().commit().ok_or_else(|| {
        ConformanceFailure::new(
            scenario,
            ConformanceCheck::Commit,
            "failed branch",
            "success branch",
        )
    })?;
    if mutation.commit() != expected_commit {
        return mismatch(
            scenario,
            ConformanceCheck::Commit,
            expected_commit,
            mutation.commit(),
        );
    }
    let source = mutation.into_source();
    if source.kind() != source_kind(scenario.operation()) {
        return mismatch(
            scenario,
            ConformanceCheck::Source,
            source_kind(scenario.operation()),
            source.kind(),
        );
    }
    Ok(())
}

pub(crate) fn expect_state(
    poll: &ScriptedPoll,
    registration: &Registration,
    expected: RegistrationState,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    let actual = poll.registration_state(registration).map_err(|error| {
        ConformanceFailure::new(
            scenario,
            ConformanceCheck::State,
            format!("{expected:?}"),
            format!("{error:?}"),
        )
    })?;
    if actual == expected {
        Ok(())
    } else {
        mismatch(scenario, ConformanceCheck::State, expected, actual)
    }
}

pub(crate) fn expect_stale(
    poll: &ScriptedPoll,
    registration: &Registration,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    match poll.registration_state(registration) {
        Err(Error::Stale {
            registration: actual,
        }) if actual == registration.id() => Ok(()),
        actual => mismatch(
            scenario,
            ConformanceCheck::State,
            format!("Stale({:?})", registration.id()),
            format!("{actual:?}"),
        ),
    }
}

pub(crate) fn expect_backend(
    poll: &ScriptedPoll,
    registration: RegistrationId,
    expected: ScriptedBackendState,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    let actual = poll.backend_state(registration);
    if actual == expected {
        Ok(())
    } else {
        mismatch(scenario, ConformanceCheck::State, expected, actual)
    }
}

pub(crate) fn expect_retained_capacity(
    poll: &mut ScriptedPoll,
    source: &UnixStream,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    let calls = poll.calls().len();
    let result = poll.register(
        source,
        zio::Key::new(9_999),
        Interest::READABLE | Interest::WRITABLE,
        Mode::Level,
    );
    let Err(error) = result else {
        return mismatch(
            scenario,
            ConformanceCheck::CapacityRetention,
            "fixed-capacity rejection",
            "registration succeeded",
        );
    };
    let (error, registration) = error.into_parts();
    if poll.calls().len() != calls {
        return mismatch(
            scenario,
            ConformanceCheck::CapacityRetention,
            "capacity rejected before backend",
            "backend call",
        );
    }
    if registration.is_some() {
        return mismatch(
            scenario,
            ConformanceCheck::IntoParts,
            "no returned capability",
            "unexpected handle",
        );
    }
    match error {
        Error::Capacity {
            kind: CapacityKind::Registration,
            limit: 1,
            reason: CapacityReason::Exhausted,
            ..
        } => Ok(()),
        actual => mismatch(
            scenario,
            ConformanceCheck::CapacityRetention,
            "Capacity { limit: 1 }",
            format!("{actual:?}"),
        ),
    }
}

pub(crate) fn finish(
    poll: &ScriptedPoll,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    poll.finish().map_err(|error| {
        ConformanceFailure::new(
            scenario,
            ConformanceCheck::Script,
            "fully consumed script",
            error.to_string(),
        )
    })
}

pub(crate) fn mismatch<T>(
    scenario: MutationScenario,
    check: ConformanceCheck,
    expected: impl Debug,
    actual: impl Debug,
) -> Result<T, ConformanceFailure> {
    Err(ConformanceFailure::new(
        scenario,
        check,
        format!("{expected:?}"),
        format!("{actual:?}"),
    ))
}
