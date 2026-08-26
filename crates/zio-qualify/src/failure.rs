//! Structured qualification failures.

use core::fmt;

use crate::{Implementation, Scenario};

/// Harness phase associated with a qualification failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualificationPhase {
    /// Candidate capability discovery.
    Capability,
    /// Fixture or candidate construction.
    Setup,
    /// Positive quiet window before fixture activation.
    Quiescence,
    /// Transitioning the fixture into the requested ready state.
    Activation,
    /// Waiting for readiness.
    Wait,
    /// Number of matching events returned by one wait.
    Cardinality,
    /// Contract validation.
    Contract,
    /// Native level redelivery.
    LevelDelivery,
    /// Native one-shot disarm.
    Disarm,
    /// Explicit one-shot rearm.
    Rearm,
    /// Corresponding nonblocking operation.
    Operation,
    /// Explicit registration cleanup.
    Cleanup,
}

/// One structured candidate/scenario failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualificationFailure {
    implementation: Implementation,
    scenario: Scenario,
    phase: QualificationPhase,
    expected: String,
    actual: String,
}

impl QualificationFailure {
    pub(crate) fn new(
        implementation: Implementation,
        scenario: Scenario,
        phase: QualificationPhase,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            implementation,
            scenario,
            phase,
            expected: expected.into(),
            actual: actual.into(),
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

    /// Returns the failed phase.
    pub const fn phase(&self) -> QualificationPhase {
        self.phase
    }

    /// Returns the declared expectation.
    pub fn expected(&self) -> &str {
        &self.expected
    }

    /// Returns the observed outcome.
    pub fn actual(&self) -> &str {
        &self.actual
    }
}

impl fmt::Display for QualificationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {} {:?}: expected {}, observed {}",
            self.implementation.name(),
            self.scenario.name(),
            self.phase,
            self.expected,
            self.actual
        )
    }
}

impl std::error::Error for QualificationFailure {}
