//! Stable-release evidence validator integration.

#![cfg(target_os = "linux")]

use std::{error::Error as StdError, path::PathBuf, process::Command};

#[test]
fn incomplete_or_stale_release_evidence_is_rejected() -> Result<(), Box<dyn StdError>> {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("scripts")
        .join("test-qualify-1.0");
    let output = Command::new("bash").arg(script).output()?;
    if !output.status.success() {
        return Err(format!(
            "stable evidence validator test failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    if !stdout.contains("stable 1.0 evidence validator tests passed") {
        return Err("stable evidence validator omitted its success receipt".into());
    }
    Ok(())
}
