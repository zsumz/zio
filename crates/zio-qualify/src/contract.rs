//! Candidate-independent observation contracts.

use crate::{ExpectedObservation, Interest, Observation, Scenario};

/// Returns the contract for one scenario.
///
/// The result depends only on the scenario, never on the candidate.
pub const fn expectation_for(scenario: Scenario) -> ExpectedObservation {
    match scenario.interest() {
        Interest::Readable => ExpectedObservation::new(
            Observation::READABLE,
            Observation::EMPTY,
            Observation::READABLE,
        ),
        Interest::Writable => ExpectedObservation::new(
            Observation::WRITABLE,
            Observation::EMPTY,
            Observation::WRITABLE,
        ),
    }
}
