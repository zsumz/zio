//! Register mutation conformance evidence.

use zio_testkit::{
    REGISTER_APPLIED, REGISTER_NOT_APPLIED, REGISTER_SUCCESS, REGISTER_UNKNOWN, run_scenario,
};

#[test]
fn register_branches_preserve_error_state_and_capabilities()
-> Result<(), Box<dyn std::error::Error>> {
    for scenario in [
        REGISTER_SUCCESS,
        REGISTER_NOT_APPLIED,
        REGISTER_APPLIED,
        REGISTER_UNKNOWN,
    ] {
        run_scenario(scenario)?;
    }
    Ok(())
}
