//! Calibration policy tests.

use super::{
    calibration::calibrate,
    measure::{Captured, CapturedMetric, Counts, Resources},
};

#[test]
fn calibration_scales_from_a_stable_probe() -> Result<(), String> {
    let result = calibrate(10, 100_000_000, 1_000_000, |iterations| {
        Ok(sample(iterations, 10_000))
    })?;
    assert_eq!(result.required_iterations, 10_000);
    assert_eq!(result.achieved_elapsed_ns, 100_000_000);
    Ok(())
}

fn sample(iterations: usize, ns_per_operation: u128) -> Captured {
    Captured {
        counts: Counts {
            operations: u64::try_from(iterations).unwrap_or(u64::MAX),
            events: 0,
        },
        metric: CapturedMetric::Timing {
            elapsed_ns: (iterations as u128).saturating_mul(ns_per_operation),
        },
        resources: Resources::default(),
    }
}
