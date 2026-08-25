//! Structured wake-conformance diagnostics.

use std::fmt;

use crate::WakeScenario;

/// Contract checkpoint that rejected a wake observation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WakeCheck {
    /// The poller, source, or event destination could not be created.
    Setup,
    /// Waker configuration or cloning diverged.
    Configuration,
    /// Triggering a wake failed.
    Trigger,
    /// A bounded or nonblocking wait failed.
    Wait,
    /// Delivered events differed from the expected logical batch.
    Events,
    /// A wake remained observable after it should have drained.
    Drain,
    /// Bounded test coordination did not complete in time.
    Deadline,
    /// Registration state diverged after saturated delivery.
    State,
    /// Registration cleanup failed.
    Cleanup,
}

/// One wake scenario failure with machine-readable location and observed values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WakeFailure {
    scenario: WakeScenario,
    check: WakeCheck,
    expected: String,
    actual: String,
}

impl WakeFailure {
    /// Creates a structured wake-conformance failure.
    pub fn new(
        scenario: WakeScenario,
        check: WakeCheck,
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
    pub const fn scenario(&self) -> WakeScenario {
        self.scenario
    }

    /// Returns the failed contract checkpoint.
    pub const fn check(&self) -> WakeCheck {
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

impl fmt::Display for WakeFailure {
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

impl std::error::Error for WakeFailure {}
