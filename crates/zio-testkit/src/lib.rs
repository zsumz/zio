//! Reference mutation, wake, and readiness conformance for [`zio`].
//!
//! The mutation runner uses zio's feature-gated scripted poller and checks the
//! same errors, registration capabilities, and authoritative state that a
//! downstream library observes. The native wake and readiness runners treat
//! [`zio::Poll`] as a black box through only its ordinary public API.
//!
//! ```
//! let report = zio_testkit::run_all();
//! report.into_result()?;
//! zio_testkit::run_readiness_conformance().into_result()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![deny(unsafe_code)]

mod calls;
mod delete;
mod delete_failure;
mod failure;
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
