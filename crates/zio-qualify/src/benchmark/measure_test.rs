//! Measurement summary tests.

use super::measure::distribution;

#[test]
fn stable_integer_summary() -> Result<(), String> {
    let summary = distribution(&[9, 1, 5, 3, 7])?;
    check(summary.median == 5, "median")?;
    check(summary.p95 == 9, "p95")?;
    check(summary.mad == 2, "mad")
}

fn check(condition: bool, message: &'static str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_owned())
}
