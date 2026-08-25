//! Structured readiness-conformance diagnostics.

use std::fmt;

use crate::ReadinessScenario;

/// Contract checkpoint that rejected a native readiness observation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReadinessCheck {
    /// The poller or native fixture could not be created.
    Setup,
    /// A bounded or nonblocking wait failed.
    Wait,
    /// The logical event batch had another shape or key.
    Events,
    /// A portable required readiness hint was absent.
    RequiredReadiness,
    /// A readiness hint outside the scenario's declared allowance appeared.
    AllowedReadiness,
    /// The operation associated with the readiness hint did not agree.
    Operation,
    /// Authoritative registration state diverged after delivery.
    State,
    /// A one-shot resource was delivered again without an explicit rearm.
    Disarm,
    /// Registration cleanup failed.
    Cleanup,
}

/// One readiness failure with machine-readable location and observed values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadinessFailure {
    scenario: ReadinessScenario,
    check: ReadinessCheck,
    expected: String,
    actual: String,
}

impl ReadinessFailure {
    /// Creates a structured readiness-conformance failure.
    pub fn new(
        scenario: ReadinessScenario,
        check: ReadinessCheck,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            scenario,
            check,
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Returns the failed scenario.
    pub const fn scenario(&self) -> ReadinessScenario {
        self.scenario
    }

    /// Returns the failed contract checkpoint.
    pub const fn check(&self) -> ReadinessCheck {
        self.check
    }

    /// Returns the expected observation.
    pub fn expected(&self) -> &str {
        &self.expected
    }

    /// Returns the actual observation.
    pub fn actual(&self) -> &str {
        &self.actual
    }
}

impl fmt::Display for ReadinessFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} failed {:?}: expected {}, observed {}",
            self.scenario.name(),
            self.check,
            self.expected,
            self.actual
        )
    }
}

impl std::error::Error for ReadinessFailure {}
