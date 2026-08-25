//! Aggregate mutation-suite results.

use std::fmt;

use crate::{ConformanceFailure, MutationScenario};

/// Result of one isolated mutation scenario.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseResult {
    scenario: MutationScenario,
    failure: Option<ConformanceFailure>,
}

impl CaseResult {
    pub(crate) const fn passed(scenario: MutationScenario) -> Self {
        Self {
            scenario,
            failure: None,
        }
    }

    pub(crate) fn failed(failure: ConformanceFailure) -> Self {
        Self {
            scenario: failure.scenario(),
            failure: Some(failure),
        }
    }

    /// Returns the executed scenario.
    pub const fn scenario(&self) -> MutationScenario {
        self.scenario
    }

    /// Returns whether the scenario conformed.
    pub const fn is_passed(&self) -> bool {
        self.failure.is_none()
    }

    /// Borrows the structured failure, when present.
    pub const fn failure(&self) -> Option<&ConformanceFailure> {
        self.failure.as_ref()
    }

    /// Splits this result into its scenario and optional failure.
    pub fn into_parts(self) -> (MutationScenario, Option<ConformanceFailure>) {
        (self.scenario, self.failure)
    }
}

/// Complete result of the twelve isolated mutation scenarios.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationReport {
    results: Box<[CaseResult]>,
}

impl MutationReport {
    pub(crate) fn new(results: Vec<CaseResult>) -> Self {
        Self {
            results: results.into_boxed_slice(),
        }
    }

    /// Returns every result in stable scenario order.
    pub fn results(&self) -> &[CaseResult] {
        &self.results
    }

    /// Returns the number of executed scenarios.
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Returns whether the report contains no scenario results.
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Returns the number of conforming scenarios.
    pub fn passed(&self) -> usize {
        self.results
            .iter()
            .filter(|result| result.is_passed())
            .count()
    }

    /// Iterates over structured failures in stable scenario order.
    pub fn failures(&self) -> impl Iterator<Item = &ConformanceFailure> + '_ {
        self.results.iter().filter_map(CaseResult::failure)
    }

    /// Returns whether every scenario conformed.
    pub fn is_conformant(&self) -> bool {
        self.failures().next().is_none()
    }

    /// Converts a conforming report into success without panicking.
    pub fn into_result(self) -> Result<(), Self> {
        if self.is_conformant() {
            Ok(())
        } else {
            Err(self)
        }
    }
}

impl fmt::Display for MutationReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "zio mutation conformance: {}/{} passed",
            self.passed(),
            self.len()
        )?;
        for failure in self.failures() {
            write!(formatter, "\n- {failure}")?;
        }
        Ok(())
    }
}

impl std::error::Error for MutationReport {}
