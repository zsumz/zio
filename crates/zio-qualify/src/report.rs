//! Structured qualification receipts.

use crate::{
    ConfiguredDelivery, Implementation, Observation, ProfileSupport, QualificationFailure, Scenario,
};

/// Outcome for one candidate/scenario pair.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaseOutcome {
    /// The candidate independently satisfied the contract.
    Passed,
    /// The exact profile was not run, with an explicit capability reason.
    NotRun(ProfileSupport),
    /// The candidate or harness failed one or more named phases.
    Failed(Vec<QualificationFailure>),
}

/// One candidate/scenario receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseResult {
    implementation: Implementation,
    scenario: Scenario,
    configured_delivery: Option<ConfiguredDelivery>,
    observations: Vec<Observation>,
    outcome: CaseOutcome,
}

impl CaseResult {
    pub(crate) fn new(
        implementation: Implementation,
        scenario: Scenario,
        configured_delivery: Option<ConfiguredDelivery>,
        observations: Vec<Observation>,
        outcome: CaseOutcome,
    ) -> Self {
        Self {
            implementation,
            scenario,
            configured_delivery,
            observations,
            outcome,
        }
    }

    /// Returns the candidate.
    pub const fn implementation(&self) -> Implementation {
        self.implementation
    }

    /// Returns the scenario.
    pub const fn scenario(&self) -> Scenario {
        self.scenario
    }

    /// Returns the delivery semantics configured for an executed case.
    pub const fn configured_delivery(&self) -> Option<ConfiguredDelivery> {
        self.configured_delivery
    }

    /// Returns observations retained in delivery order.
    pub fn observations(&self) -> &[Observation] {
        &self.observations
    }

    /// Returns the terminal case outcome.
    pub const fn outcome(&self) -> &CaseOutcome {
        &self.outcome
    }

    /// Returns every retained failure in execution order.
    pub fn failures(&self) -> &[QualificationFailure] {
        match &self.outcome {
            CaseOutcome::Failed(failures) => failures,
            CaseOutcome::Passed | CaseOutcome::NotRun(_) => &[],
        }
    }
}

/// Independent-candidate qualification receipt.
#[derive(Debug)]
pub struct QualificationReport {
    results: Vec<CaseResult>,
    expected_scope: ExpectedScope,
}

impl QualificationReport {
    pub(crate) const fn for_all(results: Vec<CaseResult>) -> Self {
        Self {
            results,
            expected_scope: ExpectedScope::All,
        }
    }

    pub(crate) const fn for_implementation(
        implementation: Implementation,
        results: Vec<CaseResult>,
    ) -> Self {
        Self {
            results,
            expected_scope: ExpectedScope::Implementation(implementation),
        }
    }

    /// Returns every case in stable execution order.
    pub fn results(&self) -> &[CaseResult] {
        &self.results
    }

    /// Returns whether every case in the report's declared scope is covered.
    pub fn has_required_coverage(&self) -> bool {
        if self.results.len() != self.expected_scope.case_count() {
            return false;
        }
        match self.expected_scope {
            ExpectedScope::All => Implementation::ALL
                .into_iter()
                .all(|implementation| implementation_is_covered(&self.results, implementation)),
            ExpectedScope::Implementation(implementation) => {
                implementation_is_covered(&self.results, implementation)
            }
        }
    }

    /// Returns whether required coverage exists and no case failed.
    pub fn is_conformant(&self) -> bool {
        self.has_required_coverage()
            && !self
                .results
                .iter()
                .any(|result| matches!(result.outcome(), CaseOutcome::Failed(_)))
    }

    /// Converts a conformant receipt into success while preserving failures.
    pub fn into_result(self) -> Result<(), Self> {
        if self.is_conformant() {
            Ok(())
        } else {
            Err(self)
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum ExpectedScope {
    All,
    Implementation(Implementation),
}

impl ExpectedScope {
    const fn case_count(self) -> usize {
        match self {
            Self::All => Implementation::ALL.len() * Scenario::ALL.len(),
            Self::Implementation(_) => Scenario::ALL.len(),
        }
    }
}

fn implementation_is_covered(results: &[CaseResult], implementation: Implementation) -> bool {
    Scenario::ALL.into_iter().all(|scenario| {
        let mut matches = results.iter().filter(|result| {
            result.implementation() == implementation && result.scenario() == scenario
        });
        let covered = matches.next().is_some_and(required_case_is_covered);
        covered && matches.next().is_none()
    })
}

fn required_case_is_covered(result: &CaseResult) -> bool {
    match (result.implementation(), result.scenario().profile()) {
        (Implementation::Mio, crate::DeliveryProfile::Level | crate::DeliveryProfile::OneShot) => {
            matches!(
                result.outcome(),
                CaseOutcome::NotRun(ProfileSupport::NotExposed { .. })
            ) && result.configured_delivery().is_none()
        }
        (Implementation::Polling, crate::DeliveryProfile::Level)
            if matches!(
                result.outcome(),
                CaseOutcome::NotRun(ProfileSupport::HostUnavailable { .. })
            ) =>
        {
            result.configured_delivery().is_none()
        }
        _ => {
            matches!(
                result.outcome(),
                CaseOutcome::Passed | CaseOutcome::Failed(_)
            ) && result.configured_delivery().is_some()
        }
    }
}
