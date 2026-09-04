//! Kqueue skew matrix and parser regressions.

use super::config::{Config, MATRIX, Row};

#[test]
fn default_matrix_is_exact() {
    assert_eq!(
        MATRIX,
        [
            Row::new(100_000, 64, 100, "0.1_percent"),
            Row::new(100_000, 256, 1_000, "1_percent"),
            Row::new(100_000, 256, 50_000, "50_percent"),
            Row::new(100_000, 1_024, 100_000, "100_percent"),
            Row::new(1_000_000, 1_024, 1_024, "sparse_1024"),
        ]
    );
}

#[test]
fn smoke_is_small_and_explicit() -> Result<(), String> {
    let config = Config::parse(["--smoke".into()])?;
    assert_eq!(config.rows(), &[Row::new(5, 2, 3, "smoke")]);
    assert_eq!(config.run_id, "smoke-unbound");
    Ok(())
}

#[test]
fn full_matrix_requires_an_explicit_run_id() -> Result<(), String> {
    let error = Config::parse([])
        .err()
        .ok_or_else(|| "full matrix accepted a missing run ID".to_owned())?;
    assert_eq!(error, "--run-id is required for the full matrix");
    let error = Config::parse(["--run-id".into(), "not-a-uuid".into()])
        .err()
        .ok_or_else(|| "full matrix accepted a malformed run ID".to_owned())?;
    assert_eq!(error, "--run-id must be one lowercase hyphenated UUID");
    let run_id = "01234567-89ab-4cde-8f01-23456789abcd";
    let config = Config::parse(["--run-id".into(), run_id.into()])?;
    assert_eq!(config.rows(), &MATRIX);
    assert_eq!(config.run_id, run_id);
    Ok(())
}
