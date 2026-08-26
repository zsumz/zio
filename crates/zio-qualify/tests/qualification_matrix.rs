//! Native candidate qualification-matrix checks.

use std::io;

use zio_qualify::{
    CaseOutcome, ConfiguredDelivery, DeliveryProfile, Implementation, ProfileSupport, Scenario,
    qualify_all, qualify_implementation,
};

#[test]
fn initial_observation_contracts_pass_for_every_candidate() -> Result<(), io::Error> {
    let report = qualify_all();
    for result in report
        .results()
        .iter()
        .filter(|result| result.scenario().profile() == DeliveryProfile::InitialObservation)
    {
        check(
            result.outcome() == &CaseOutcome::Passed,
            "a candidate failed the common initial-observation contract",
        )?;
        check(
            result.observations().len() == 1,
            "initial observation retained an unexpected event count",
        )?;
    }
    Ok(())
}

#[test]
fn exact_delivery_profiles_are_never_attributed_to_mio() -> Result<(), io::Error> {
    let report = qualify_all();
    for result in report.results().iter().filter(|result| {
        result.implementation() == Implementation::Mio
            && result.scenario().profile() != DeliveryProfile::InitialObservation
    }) {
        check(
            matches!(
                result.outcome(),
                CaseOutcome::NotRun(ProfileSupport::NotExposed { .. })
            ),
            "Mio was credited with a delivery profile its API does not expose",
        )?;
        check(
            result.configured_delivery().is_none(),
            "a not-run Mio profile reported configured delivery",
        )?;
    }
    Ok(())
}

#[test]
fn receipts_name_the_exact_configured_delivery() -> Result<(), io::Error> {
    let report = qualify_all();
    for result in report.results() {
        let expected = match (result.implementation(), result.scenario().profile()) {
            (
                Implementation::Zio | Implementation::ZioBorrowed,
                DeliveryProfile::InitialObservation | DeliveryProfile::Level,
            ) => Some(ConfiguredDelivery::Level),
            (Implementation::Zio | Implementation::ZioBorrowed, DeliveryProfile::OneShot)
            | (
                Implementation::Polling,
                DeliveryProfile::InitialObservation | DeliveryProfile::OneShot,
            ) => Some(ConfiguredDelivery::OneShot),
            (Implementation::Mio, DeliveryProfile::InitialObservation) => {
                Some(ConfiguredDelivery::NativeDefault)
            }
            (Implementation::Mio, DeliveryProfile::Level | DeliveryProfile::OneShot) => None,
            (Implementation::Polling, DeliveryProfile::Level) => {
                (!matches!(result.outcome(), CaseOutcome::NotRun(_)))
                    .then_some(ConfiguredDelivery::Level)
            }
        };
        check(
            result.configured_delivery() == expected,
            "receipt reported incorrect configured delivery",
        )?;
    }
    Ok(())
}

#[test]
fn qualification_report_has_no_candidate_oracle() -> Result<(), io::Error> {
    let report = qualify_all();
    check(
        report.has_required_coverage(),
        "qualification matrix lacked required coverage",
    )?;
    check(report.is_conformant(), "an executed contract failed")?;
    check(
        report.results().len() == Implementation::ALL.len() * Scenario::ALL.len(),
        "matrix omitted a candidate/scenario receipt",
    )?;
    for result in report.results() {
        if result.implementation() == Implementation::Mio
            && result.scenario().profile() != DeliveryProfile::InitialObservation
        {
            continue;
        }
        if result.implementation() == Implementation::Polling
            && result.scenario().profile() == DeliveryProfile::Level
            && matches!(
                result.outcome(),
                CaseOutcome::NotRun(ProfileSupport::HostUnavailable { .. })
            )
        {
            continue;
        }
        check(
            result.outcome() == &CaseOutcome::Passed,
            "candidate did not independently satisfy its declared contract",
        )?;
    }
    Ok(())
}

#[test]
fn implementation_reports_cover_their_explicit_scope() -> Result<(), io::Error> {
    for implementation in Implementation::ALL {
        let report = qualify_implementation(implementation);
        check(
            report.results().len() == Scenario::ALL.len(),
            "implementation report had the wrong case count",
        )?;
        check(
            report
                .results()
                .iter()
                .all(|result| result.implementation() == implementation),
            "implementation report included another candidate",
        )?;
        check(
            report.has_required_coverage(),
            "implementation report lacked required coverage",
        )?;
        check(report.is_conformant(), "implementation report failed")?;
    }
    Ok(())
}

#[test]
fn executed_delivery_profiles_have_exact_repeat_observations() -> Result<(), io::Error> {
    let report = qualify_all();
    for result in report.results().iter().filter(|result| {
        matches!(
            result.scenario().profile(),
            DeliveryProfile::Level | DeliveryProfile::OneShot
        ) && !matches!(result.outcome(), CaseOutcome::NotRun(_))
    }) {
        let expected = match result.scenario().interest() {
            zio_qualify::Interest::Readable => [
                zio_qualify::Observation::READABLE,
                zio_qualify::Observation::READABLE,
            ],
            zio_qualify::Interest::Writable => [
                zio_qualify::Observation::WRITABLE,
                zio_qualify::Observation::WRITABLE,
            ],
        };
        check(
            result.outcome() == &CaseOutcome::Passed,
            "an executed repeat-delivery case failed",
        )?;
        check(
            result.observations() == expected,
            "repeat-delivery observations did not match exactly",
        )?;
    }
    Ok(())
}

fn check(condition: bool, message: &'static str) -> Result<(), io::Error> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message))
    }
}
