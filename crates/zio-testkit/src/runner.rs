//! Isolated mutation scenario execution.

use crate::{
    CaseResult, ConformanceCheck, ConformanceFailure, MutationOperation, MutationReport,
    MutationScenario, REGISTER_SUCCESS,
};

/// Runs one mutation scenario and returns its structured contract result.
pub fn run_scenario(scenario: MutationScenario) -> Result<(), ConformanceFailure> {
    match scenario.operation() {
        MutationOperation::Register => crate::register::run(scenario),
        MutationOperation::Modify => crate::modify::run(scenario),
        MutationOperation::Delete => crate::delete::run(scenario),
    }
}

/// Runs all twelve V1 scenarios in stable order.
pub fn run_all() -> MutationReport {
    let mut results = Vec::new();
    if results
        .try_reserve_exact(MutationScenario::ALL.len())
        .is_err()
    {
        let scenario = REGISTER_SUCCESS;
        return MutationReport::new(vec![CaseResult::failed(ConformanceFailure::new(
            scenario,
            ConformanceCheck::Setup,
            "result storage",
            "allocation failure",
        ))]);
    }
    for scenario in MutationScenario::ALL {
        results.push(match run_scenario(scenario) {
            Ok(()) => CaseResult::passed(scenario),
            Err(failure) => CaseResult::failed(failure),
        });
    }
    MutationReport::new(results)
}
