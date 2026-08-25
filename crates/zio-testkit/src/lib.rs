//! Reference mutation conformance for [`zio`].
//!
//! The runner uses zio's feature-gated scripted poller and checks the same
//! public errors, registration capabilities, and authoritative state that a
//! downstream library observes.
//!
//! ```
//! let report = zio_testkit::run_all();
//! report.into_result()?;
//! # Ok::<(), zio_testkit::MutationReport>(())
//! ```

#![deny(unsafe_code)]

mod calls;
mod delete;
mod delete_failure;
mod failure;
mod modify;
mod modify_commit;
mod register;
mod register_failure;
mod report;
mod runner;
mod scenario;
mod setup;
pub mod support;
mod verify;

pub use failure::{ConformanceCheck, ConformanceFailure};
pub use report::{CaseResult, MutationReport};
pub use runner::{run_all, run_scenario};
pub use scenario::{
    Branch, DELETE_APPLIED, DELETE_NOT_APPLIED, DELETE_SUCCESS, DELETE_UNKNOWN, MODIFY_APPLIED,
    MODIFY_NOT_APPLIED, MODIFY_SUCCESS, MODIFY_UNKNOWN, MutationOperation, MutationScenario,
    REGISTER_APPLIED, REGISTER_NOT_APPLIED, REGISTER_SUCCESS, REGISTER_UNKNOWN,
};
