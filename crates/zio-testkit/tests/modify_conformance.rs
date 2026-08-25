//! Modify mutation conformance evidence.

use zio_testkit::{
    MODIFY_APPLIED, MODIFY_NOT_APPLIED, MODIFY_SUCCESS, MODIFY_UNKNOWN, run_scenario,
};

#[test]
fn modify_branches_preserve_error_state_and_retries() -> Result<(), Box<dyn std::error::Error>> {
    for scenario in [
        MODIFY_SUCCESS,
        MODIFY_NOT_APPLIED,
        MODIFY_APPLIED,
        MODIFY_UNKNOWN,
    ] {
        run_scenario(scenario)?;
    }
    Ok(())
}
