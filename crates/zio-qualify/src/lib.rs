//! Workspace-private, contract-driven readiness qualification.
//!
//! `zio`'s owned and borrowed tiers, Mio, and `polling` are independent
//! candidates. A passing observation means that candidate satisfied the
//! declared contract; it never means that the candidate agreed with another
//! library.

mod benchmark;
mod candidate;
mod contract;
mod failure;
mod fixture;
mod mio_candidate;
mod model;
mod observation;
mod observe;
mod polling_candidate;
mod polling_registration;
mod report;
mod runner;
mod zio_borrowed_candidate;
mod zio_candidate;

pub use benchmark::run_perf;
#[cfg(feature = "allocation-metrics")]
pub use benchmark::run_perf_alloc;
pub use contract::expectation_for;
pub use failure::{QualificationFailure, QualificationPhase};
pub use model::{
    ConfiguredDelivery, DeliveryProfile, Implementation, Interest, ProfileSupport, Scenario,
};
pub use observation::{ContractViolation, ExpectedObservation, Observation};
pub use report::{CaseOutcome, CaseResult, QualificationReport};
pub use runner::{qualify_all, qualify_implementation, qualify_scenario};

#[cfg(test)]
mod polling_registration_test;
#[cfg(test)]
mod report_test;
#[cfg(test)]
mod runner_test;
