//! Delete error capability checks.

use zio::{DeleteError, Registration, RegistrationId};

use crate::{
    ConformanceCheck, ConformanceFailure, MutationScenario,
    verify::{expect_mutation, mismatch},
};

pub(crate) fn expect_failed_delete(
    result: Result<(), DeleteError>,
    expected: RegistrationId,
    scenario: MutationScenario,
) -> Result<Registration, ConformanceFailure> {
    let error = match result {
        Ok(()) => {
            return mismatch(
                scenario,
                ConformanceCheck::Result,
                "deletion failure",
                "success",
            );
        }
        Err(error) => error,
    };
    if error.registration().id() != expected {
        return mismatch(
            scenario,
            ConformanceCheck::Handle,
            expected,
            error.registration().id(),
        );
    }
    let (cause, registration) = error.into_parts();
    if registration.id() != expected {
        return mismatch(
            scenario,
            ConformanceCheck::IntoParts,
            expected,
            registration.id(),
        );
    }
    expect_mutation(scenario, cause)?;
    Ok(registration)
}

pub(crate) fn unexpected_delete(
    scenario: MutationScenario,
    error: &DeleteError,
) -> ConformanceFailure {
    ConformanceFailure::new(
        scenario,
        ConformanceCheck::Result,
        "successful deletion",
        error.to_string(),
    )
}
