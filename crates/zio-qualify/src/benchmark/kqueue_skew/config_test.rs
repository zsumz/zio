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
    Ok(())
}
