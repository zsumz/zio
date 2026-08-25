//! Delete mutation conformance evidence.

use zio_testkit::{
    DELETE_APPLIED, DELETE_NOT_APPLIED, DELETE_SUCCESS, DELETE_UNKNOWN, run_scenario,
};

#[test]
fn delete_branches_preserve_returned_handles_and_retries() -> Result<(), Box<dyn std::error::Error>>
{
    for scenario in [
        DELETE_SUCCESS,
        DELETE_NOT_APPLIED,
        DELETE_APPLIED,
        DELETE_UNKNOWN,
    ] {
        run_scenario(scenario)?;
    }
    Ok(())
}
