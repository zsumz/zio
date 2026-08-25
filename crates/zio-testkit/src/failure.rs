//! Structured conformance diagnostics.

use std::fmt;

use crate::MutationScenario;

/// Contract checkpoint that rejected an observation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConformanceCheck {
    /// The scripted poll or source fixture could not be created.
    Setup,
    /// Success and failure did not match the selected branch.
    Result,
    /// The error named another operation.
    Operation,
    /// The mutation reported another commit status.
    Commit,
    /// The underlying I/O error was not preserved.
    Source,
    /// Authoritative portable or scripted backend state diverged.
    State,
    /// A move-only registration was lost, invented, or replaced.
    Handle,
    /// A retained registration stopped consuming fixed capacity.
    CapacityRetention,
    /// A returned capability could not perform the required retry.
    Retry,
    /// Splitting an error lost one of its owned parts.
    IntoParts,
    /// The scripted backend observed missing, extra, or misordered calls.
    Script,
}

/// One scenario failure with machine-readable location and human-readable values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConformanceFailure {
    scenario: MutationScenario,
    check: ConformanceCheck,
    expected: String,
    actual: String,
}

impl ConformanceFailure {
    /// Creates a structured scenario failure.
    pub fn new(
        scenario: MutationScenario,
        check: ConformanceCheck,
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
    pub const fn scenario(&self) -> MutationScenario {
        self.scenario
    }

    /// Returns the failed contract checkpoint.
    pub const fn check(&self) -> ConformanceCheck {
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

impl fmt::Display for ConformanceFailure {
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

impl std::error::Error for ConformanceFailure {}
