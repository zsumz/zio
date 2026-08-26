//! Metric-isolated timing, allocation, descriptor, and distribution capture.

use std::{fs, path::PathBuf, time::Instant};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Counts {
    pub(crate) operations: u64,
    pub(crate) events: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Metric {
    Timing,
    Allocation,
}

impl Metric {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Timing => "timing",
            Self::Allocation => "allocation",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Allocations {
    pub(crate) count_total: u64,
    pub(crate) count_current: i64,
    pub(crate) count_peak: u64,
    pub(crate) bytes_total: u64,
    pub(crate) bytes_current: i64,
    pub(crate) bytes_peak: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CapturedMetric {
    Warmup,
    Timing {
        elapsed_ns: u128,
    },
    #[cfg_attr(
        not(feature = "allocation-metrics"),
        allow(dead_code, reason = "allocation sample construction is feature-gated")
    )]
    Allocation(Allocations),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Captured {
    pub(crate) counts: Counts,
    pub(crate) metric: CapturedMetric,
}

pub(crate) fn capture(
    iterations: usize,
    metric: Option<Metric>,
    mut iteration: impl FnMut() -> Result<u64, String>,
) -> Result<Captured, String> {
    match metric {
        None => run_loop(iterations, &mut iteration).map(|counts| Captured {
            counts,
            metric: CapturedMetric::Warmup,
        }),
        Some(Metric::Timing) => {
            let started = Instant::now();
            run_loop(iterations, &mut iteration).map(|counts| Captured {
                counts,
                metric: CapturedMetric::Timing {
                    elapsed_ns: started.elapsed().as_nanos(),
                },
            })
        }
        Some(Metric::Allocation) => capture_allocations(iterations, &mut iteration),
    }
}

fn capture_allocations(
    iterations: usize,
    iteration: &mut impl FnMut() -> Result<u64, String>,
) -> Result<Captured, String> {
    #[cfg(feature = "allocation-metrics")]
    {
        let mut result = Ok(Counts::default());
        let measured = allocation_counter::measure(|| {
            result = run_loop(iterations, iteration);
        });
        result.map(|counts| Captured {
            counts,
            metric: CapturedMetric::Allocation(Allocations {
                count_total: measured.count_total,
                count_current: measured.count_current,
                count_peak: measured.count_max,
                bytes_total: measured.bytes_total,
                bytes_current: measured.bytes_current,
                bytes_peak: measured.bytes_max,
            }),
        })
    }
    #[cfg(not(feature = "allocation-metrics"))]
    {
        let _ = (iterations, iteration);
        Err("allocation capture requires the `allocation-metrics` feature".to_owned())
    }
}

fn run_loop(
    iterations: usize,
    iteration: &mut impl FnMut() -> Result<u64, String>,
) -> Result<Counts, String> {
    let mut counts = Counts::default();
    for _ in 0..iterations {
        counts.events = counts.events.saturating_add(iteration()?);
        counts.operations = counts.operations.saturating_add(1);
    }
    Ok(counts)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FdProbe {
    Available(PathBuf),
    Unavailable(&'static str),
}

impl FdProbe {
    pub(crate) fn discover() -> Self {
        for path in ["/proc/self/fd", "/dev/fd"] {
            if count_entries(path).is_ok() {
                return Self::Available(PathBuf::from(path));
            }
        }
        Self::Unavailable("neither /proc/self/fd nor /dev/fd is readable")
    }

    pub(crate) fn count(&self) -> Option<usize> {
        match self {
            Self::Available(path) => count_entries(path).ok(),
            Self::Unavailable(_) => None,
        }
    }

    pub(crate) fn path(&self) -> Option<&str> {
        match self {
            Self::Available(path) => path.to_str(),
            Self::Unavailable(_) => None,
        }
    }

    pub(crate) const fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Available(_) => None,
            Self::Unavailable(reason) => Some(reason),
        }
    }
}

fn count_entries(path: impl AsRef<std::path::Path>) -> Result<usize, std::io::Error> {
    let mut count = 0_usize;
    for entry in fs::read_dir(path)? {
        entry?;
        count = count.saturating_add(1);
    }
    Ok(count)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Distribution {
    pub(crate) median: u128,
    pub(crate) p95: u128,
    pub(crate) mad: u128,
    pub(crate) minimum: u128,
    pub(crate) maximum: u128,
}

pub(crate) fn distribution(values: &[u128]) -> Result<Distribution, String> {
    if values.is_empty() {
        return Err("cannot summarize an empty sample set".to_owned());
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let median_value = median(&sorted)?;
    let mut deviations: Vec<_> = sorted
        .iter()
        .map(|value| value.abs_diff(median_value))
        .collect();
    deviations.sort_unstable();
    let mad = median(&deviations)?;
    let rank = sorted.len().saturating_mul(95).div_ceil(100).max(1);
    let p95 = value_at(&sorted, rank - 1)?;
    Ok(Distribution {
        median: median_value,
        p95,
        mad,
        minimum: value_at(&sorted, 0)?,
        maximum: value_at(&sorted, sorted.len() - 1)?,
    })
}

fn median(sorted: &[u128]) -> Result<u128, String> {
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        value_at(sorted, middle)
    } else {
        Ok(value_at(sorted, middle - 1)?.midpoint(value_at(sorted, middle)?))
    }
}

fn value_at(values: &[u128], index: usize) -> Result<u128, String> {
    values
        .get(index)
        .copied()
        .ok_or_else(|| "sample index invariant failed".to_owned())
}
