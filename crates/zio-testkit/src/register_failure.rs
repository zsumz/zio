//! Registration handle and owned-error checks.

use zio::{Error, RegisterError, Registration, RegistrationId};

use crate::{ConformanceCheck, ConformanceFailure, MutationScenario, verify::mismatch};

pub(crate) fn expect_failed_register(
    result: Result<Registration, RegisterError>,
    expected: Option<RegistrationId>,
    scenario: MutationScenario,
) -> Result<(Error, Option<Registration>), ConformanceFailure> {
    let error = match result {
        Ok(registration) => {
            return mismatch(
                scenario,
                ConformanceCheck::Result,
                "registration failure",
                format!("success with {:?}", registration.id()),
            );
        }
        Err(error) => error,
    };
    let retained = error.registration().map(|registration| registration.id());
    if retained != expected {
        return mismatch(scenario, ConformanceCheck::Handle, expected, retained);
    }
    let (cause, registration) = error.into_parts();
    let owned = registration.as_ref().map(Registration::id);
    if owned != expected {
        return mismatch(scenario, ConformanceCheck::IntoParts, expected, owned);
    }
    Ok((cause, registration))
}

pub(crate) fn expect_handle_id(
    registration: &Registration,
    expected: RegistrationId,
    scenario: MutationScenario,
) -> Result<(), ConformanceFailure> {
    if registration.id() == expected {
        Ok(())
    } else {
        mismatch(
            scenario,
            ConformanceCheck::Handle,
            expected,
            registration.id(),
        )
    }
}
