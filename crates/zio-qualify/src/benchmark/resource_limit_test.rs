//! Descriptor-limit parser tests.

use super::resource_limit::parse_limit;

#[test]
fn parses_numeric_and_unlimited_limits() -> Result<(), String> {
    if parse_limit("256") == Some(256) && parse_limit("unlimited") == Some(u64::MAX) {
        Ok(())
    } else {
        Err("soft limit parsing changed".to_owned())
    }
}
