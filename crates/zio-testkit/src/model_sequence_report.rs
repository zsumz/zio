//! Aggregate results for the deterministic model-sequence corpus.

use std::fmt;

use crate::ModelSequenceFailure;

/// Result of one deterministic model sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSequenceCaseResult {
    seed: u64,
    steps: usize,
    failure: Option<ModelSequenceFailure>,
}

impl ModelSequenceCaseResult {
    pub(crate) const fn passed(seed: u64, steps: usize) -> Self {
        Self {
            seed,
            steps,
            failure: None,
        }
    }

    pub(crate) fn failed(failure: ModelSequenceFailure) -> Self {
        Self {
            seed: failure.seed(),
            steps: failure.trace().len(),
            failure: Some(failure),
        }
    }

    /// Returns the deterministic generator seed.
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns the executed action count.
    pub const fn steps(&self) -> usize {
        self.steps
    }

    /// Returns whether this sequence conformed.
    pub const fn is_passed(&self) -> bool {
        self.failure.is_none()
    }

    /// Borrows the first divergence, when present.
    pub const fn failure(&self) -> Option<&ModelSequenceFailure> {
        self.failure.as_ref()
    }
}

/// Complete result of the stable deterministic corpus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSequenceReport {
    results: Box<[ModelSequenceCaseResult]>,
    coverage_failure: Option<ModelSequenceFailure>,
}

impl ModelSequenceReport {
    pub(crate) fn new(
        results: Vec<ModelSequenceCaseResult>,
        coverage_failure: Option<ModelSequenceFailure>,
    ) -> Self {
        Self {
            results: results.into_boxed_slice(),
            coverage_failure,
        }
    }

    /// Returns every sequence result in stable corpus order.
    pub fn results(&self) -> &[ModelSequenceCaseResult] {
        &self.results
    }

    /// Returns the number of executed seeds.
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Returns whether the corpus contains no seed results.
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Returns whether every required transition class was generated.
    pub const fn is_coverage_complete(&self) -> bool {
        self.coverage_failure.is_none()
    }

    /// Returns the corpus-coverage failure, when present.
    pub const fn coverage_failure(&self) -> Option<&ModelSequenceFailure> {
        self.coverage_failure.as_ref()
    }

    /// Iterates over sequence divergences in stable seed order.
    pub fn failures(&self) -> impl Iterator<Item = &ModelSequenceFailure> + '_ {
        self.results
            .iter()
            .filter_map(ModelSequenceCaseResult::failure)
            .chain(self.coverage_failure.iter())
    }

    /// Returns whether every sequence and the corpus coverage conformed.
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

impl fmt::Display for ModelSequenceReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let passed = self
            .results
            .iter()
            .filter(|result| result.is_passed())
            .count();
        write!(
            formatter,
            "zio model-sequence conformance: {passed}/{} seeds passed",
            self.len()
        )?;
        for failure in self.failures() {
            write!(formatter, "\n- {failure}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ModelSequenceReport {}
