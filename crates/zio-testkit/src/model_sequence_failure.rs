//! Stable replay diagnostics for deterministic model sequences.

use std::fmt;

#[derive(Debug)]
pub(crate) struct Divergence {
    pub(crate) check: ModelSequenceCheck,
    pub(crate) expected: String,
    pub(crate) actual: String,
}

impl Divergence {
    pub(crate) fn new(
        check: ModelSequenceCheck,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            check,
            expected: expected.into(),
            actual: actual.into(),
        }
    }
}

/// Model-sequence checkpoint that rejected an observation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ModelSequenceCheck {
    /// Action generation or fixture setup failed.
    Setup,
    /// A generated action violated its reference-model precondition.
    Precondition,
    /// Success or failure differed from the planned outcome.
    Result,
    /// A mutation error reported another operation or commit status.
    Commit,
    /// A returned or copied handle named another registration.
    Handle,
    /// A registration generation was repeated after retirement.
    Generation,
    /// Portable registration state diverged from the reference model.
    State,
    /// Scripted backend state diverged from the reference model.
    Backend,
    /// The backend observed missing, extra, or misordered work.
    Calls,
    /// The deterministic corpus omitted a required transition class.
    Coverage,
}

/// Stage at which a model sequence first diverged.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ModelSequencePhase {
    /// The replay fixture could not be constructed.
    Setup,
    /// One generated action diverged at its zero-based index.
    Action {
        /// Index in the generated action program.
        index: usize,
    },
    /// Final script-consumption checks diverged after all actions.
    Finalize,
    /// Aggregate corpus coverage was incomplete.
    Coverage,
}

/// First divergence for one replayable deterministic sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelSequenceFailure {
    seed: u64,
    phase: ModelSequencePhase,
    trace: Box<[String]>,
    check: ModelSequenceCheck,
    expected: String,
    actual: String,
}

impl ModelSequenceFailure {
    pub(crate) fn new(
        seed: u64,
        phase: ModelSequencePhase,
        trace: &[String],
        check: ModelSequenceCheck,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            seed,
            phase,
            trace: trace.to_vec().into_boxed_slice(),
            check,
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Returns the exact deterministic generator seed.
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    /// Returns the stage of the first divergence.
    pub const fn phase(&self) -> ModelSequencePhase {
        self.phase
    }

    /// Returns the failing action index, if the divergence occurred in one.
    pub const fn step(&self) -> Option<usize> {
        match self.phase {
            ModelSequencePhase::Action { index } => Some(index),
            ModelSequencePhase::Setup
            | ModelSequencePhase::Finalize
            | ModelSequencePhase::Coverage => None,
        }
    }

    /// Returns attempted generated actions through the first divergence.
    ///
    /// Setup and aggregate coverage failures have an empty action prefix. The
    /// exact seed remains the replay authority; names are diagnostic context.
    pub fn trace(&self) -> &[String] {
        &self.trace
    }

    /// Returns the failed checkpoint.
    pub const fn check(&self) -> ModelSequenceCheck {
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

impl fmt::Display for ModelSequenceFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "model sequence seed {:#018x} phase {:?} failed {:?}: expected {}, observed {}; replay",
            self.seed, self.phase, self.check, self.expected, self.actual
        )?;
        for action in &self.trace {
            write!(formatter, " -> {action}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ModelSequenceFailure {}
