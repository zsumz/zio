//! Consumer-visible mutation, wake, and readiness conformance for [`zio`].
//! Native wake and readiness checks use only the public [`zio::Poll`] API.

#![deny(unsafe_code)]

mod calls;
mod delete;
mod delete_failure;
mod failure;
mod model_sequence;
mod model_sequence_coverage;
mod model_sequence_expect;
mod model_sequence_failure;
mod model_sequence_generate;
mod model_sequence_model;
#[cfg(test)]
mod model_sequence_preflight_test;
mod model_sequence_probe;
mod model_sequence_report;
mod model_sequence_runner;
#[cfg(test)]
mod model_sequence_runner_test;
mod model_sequence_step;
mod model_sequence_verify;
mod modify;
mod modify_commit;
mod readiness_expectation;
mod readiness_failure;
mod readiness_pending;
mod readiness_pipe;
mod readiness_report;
mod readiness_runner;
mod readiness_scenario;
mod readiness_split;
mod readiness_stream;
mod readiness_verify;
mod register;
mod register_failure;
mod report;
mod runner;
mod scenario;
mod setup;
pub mod support;
mod verify;
mod wake_config;
mod wake_delivery;
mod wake_failure;
mod wake_report;
mod wake_runner;
mod wake_saturation;
mod wake_scenario;
mod wake_verify;

pub use failure::{ConformanceCheck, ConformanceFailure};
pub use model_sequence::{
    MODEL_SEQUENCE_DISARM_REARM_SEED, MODEL_SEQUENCE_OUTCOME_MATRIX_SEED,
    MODEL_SEQUENCE_SENTINEL_SEEDS, MODEL_SEQUENCE_STALE_REUSE_SEED,
    MODEL_SEQUENCE_WRONG_POLLER_SEED,
};
pub use model_sequence_failure::{ModelSequenceCheck, ModelSequenceFailure, ModelSequencePhase};
pub use model_sequence_report::{ModelSequenceCaseResult, ModelSequenceReport};
pub use model_sequence_runner::{run_model_sequence, run_model_sequences};
pub use readiness_failure::{ReadinessCheck, ReadinessFailure};
pub use readiness_report::{ReadinessCaseResult, ReadinessReport};
pub use readiness_runner::{run_readiness_conformance, run_readiness_scenario};
pub use readiness_scenario::{ReadinessFixture, ReadinessScenario};
pub use report::{CaseResult, MutationReport};
pub use runner::{run_all, run_scenario};
pub use scenario::{
    Branch, DELETE_APPLIED, DELETE_NOT_APPLIED, DELETE_SUCCESS, DELETE_UNKNOWN, MODIFY_APPLIED,
    MODIFY_NOT_APPLIED, MODIFY_SUCCESS, MODIFY_UNKNOWN, MutationOperation, MutationScenario,
    REGISTER_APPLIED, REGISTER_NOT_APPLIED, REGISTER_SUCCESS, REGISTER_UNKNOWN,
};
pub use wake_failure::{WakeCheck, WakeFailure};
pub use wake_report::{WakeCaseResult, WakeReport};
pub use wake_runner::{run_wake_conformance, run_wake_scenario};
pub use wake_scenario::{
    WAKE_CAPACITY_ONE_SATURATION, WAKE_CLONE_ACROSS_WAIT, WAKE_CONFLICTING_KEY,
    WAKE_PRE_WAIT_STORM, WAKE_SAME_KEY_CLONES, WakeScenario,
};
