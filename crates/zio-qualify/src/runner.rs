//! Independent fixture and candidate execution.

use crate::{
    CaseOutcome, CaseResult, Implementation, Observation, ProfileSupport, QualificationFailure,
    QualificationPhase, QualificationReport, Scenario,
    candidate::{Candidate, CandidateSession},
    fixture::Fixture,
    mio_candidate::MioCandidate,
    model::RegistrationSpec,
    observe::{observe_profile, verify_initial_quiet},
    polling_candidate::PollingCandidate,
    zio_borrowed_candidate::ZioBorrowedCandidate,
    zio_candidate::ZioCandidate,
};

const KEY: usize = 7_001;

/// Qualifies every candidate and scenario in stable order.
pub fn qualify_all() -> QualificationReport {
    let mut results = Vec::with_capacity(Implementation::ALL.len() * Scenario::ALL.len());
    for implementation in Implementation::ALL {
        for scenario in Scenario::ALL {
            results.push(qualify_scenario(implementation, scenario));
        }
    }
    QualificationReport::for_all(results)
}

/// Qualifies every scenario for one candidate.
pub fn qualify_implementation(implementation: Implementation) -> QualificationReport {
    let results = Scenario::ALL
        .into_iter()
        .map(|scenario| qualify_scenario(implementation, scenario))
        .collect();
    QualificationReport::for_implementation(implementation, results)
}

/// Qualifies one candidate against one declared scenario contract.
pub fn qualify_scenario(implementation: Implementation, scenario: Scenario) -> CaseResult {
    match implementation {
        Implementation::Zio => run::<ZioCandidate>(implementation, scenario),
        Implementation::ZioBorrowed => run::<ZioBorrowedCandidate>(implementation, scenario),
        Implementation::Mio => run::<MioCandidate>(implementation, scenario),
        Implementation::Polling => run::<PollingCandidate>(implementation, scenario),
    }
}

pub(crate) fn run<C: Candidate>(implementation: Implementation, scenario: Scenario) -> CaseResult {
    let support = match C::support(scenario.profile()) {
        Ok(support) => support,
        Err(actual) => {
            return failed(
                implementation,
                scenario,
                None,
                Vec::new(),
                QualificationPhase::Capability,
                "candidate capability discovery",
                actual,
            );
        }
    };
    if support != ProfileSupport::Native {
        return CaseResult::new(
            implementation,
            scenario,
            None,
            Vec::new(),
            CaseOutcome::NotRun(support),
        );
    }

    let mut fixture = match Fixture::new(scenario) {
        Ok(fixture) => fixture,
        Err(error) => {
            return failed(
                implementation,
                scenario,
                None,
                Vec::new(),
                QualificationPhase::Setup,
                "fresh independent Unix stream fixture",
                error.to_string(),
            );
        }
    };
    let configured_delivery = C::configured_delivery(scenario.profile());
    let spec = RegistrationSpec {
        key: KEY,
        interest: scenario.interest(),
        profile: scenario.profile(),
    };
    let (source, mut driver) = fixture.parts();
    let mut session = match C::register(source, spec) {
        Ok(session) => session,
        Err(actual) => {
            return failed(
                implementation,
                scenario,
                Some(configured_delivery),
                Vec::new(),
                QualificationPhase::Setup,
                "registered fixture",
                actual,
            );
        }
    };

    let mut observations = Vec::with_capacity(2);
    let mut failures = Vec::new();
    if let Err(cause) = verify_initial_quiet(&mut session, implementation, scenario) {
        failures.push(cause);
    }
    let activated = match driver.activate() {
        Ok(()) => true,
        Err(error) => {
            failures.push(failure(
                implementation,
                scenario,
                QualificationPhase::Activation,
                "fixture transitioned from quiet to ready",
                error.to_string(),
            ));
            false
        }
    };
    if activated
        && let Err(cause) =
            observe_profile(&mut session, implementation, scenario, &mut observations)
    {
        failures.push(cause);
    }
    if let Err(actual) = session.delete() {
        failures.push(failure(
            implementation,
            scenario,
            QualificationPhase::Cleanup,
            "deleted registration before fixture drop",
            actual,
        ));
    }
    if let Err(error) = driver.verify_operation() {
        failures.push(failure(
            implementation,
            scenario,
            QualificationPhase::Operation,
            "matching nonblocking operation returns fixture bytes",
            error.to_string(),
        ));
    }

    if failures.is_empty() {
        CaseResult::new(
            implementation,
            scenario,
            Some(configured_delivery),
            observations,
            CaseOutcome::Passed,
        )
    } else {
        CaseResult::new(
            implementation,
            scenario,
            Some(configured_delivery),
            observations,
            CaseOutcome::Failed(failures),
        )
    }
}

fn failure(
    implementation: Implementation,
    scenario: Scenario,
    phase: QualificationPhase,
    expected: impl Into<String>,
    actual: impl Into<String>,
) -> QualificationFailure {
    QualificationFailure::new(implementation, scenario, phase, expected, actual)
}

fn failed(
    implementation: Implementation,
    scenario: Scenario,
    configured_delivery: Option<crate::ConfiguredDelivery>,
    observations: Vec<Observation>,
    phase: QualificationPhase,
    expected: impl Into<String>,
    actual: impl Into<String>,
) -> CaseResult {
    CaseResult::new(
        implementation,
        scenario,
        configured_delivery,
        observations,
        CaseOutcome::Failed(vec![failure(
            implementation,
            scenario,
            phase,
            expected,
            actual,
        )]),
    )
}
