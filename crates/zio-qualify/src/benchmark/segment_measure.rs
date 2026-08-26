//! Pause-free isolation of one segment inside a reusable lifecycle fixture.

use std::time::Instant;

use super::measure::{Allocations, Captured, CapturedMetric, Counts, Metric, Resources};

pub(crate) fn capture_segmented(
    iterations: usize,
    operations_per_iteration: u64,
    metric: Option<Metric>,
    mut iteration: impl FnMut(&mut Segment) -> Result<u64, String>,
) -> Result<Captured, String> {
    let mut segment = Segment::new(metric);
    let mut counts = Counts::default();
    for _ in 0..iterations {
        counts.events = counts.events.saturating_add(iteration(&mut segment)?);
        counts.operations = counts.operations.saturating_add(operations_per_iteration);
    }
    Ok(Captured {
        counts,
        metric: segment.finish(),
        resources: Resources::default(),
    })
}

pub(crate) struct Segment {
    metric: Option<Metric>,
    elapsed_ns: u128,
    allocations: Allocations,
}

impl Segment {
    fn new(metric: Option<Metric>) -> Self {
        Self {
            metric,
            elapsed_ns: 0,
            allocations: Allocations::default(),
        }
    }

    pub(crate) fn measure<T>(
        &mut self,
        work: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        match self.metric {
            None => work(),
            Some(Metric::Timing) => {
                let started = Instant::now();
                let result = work();
                self.elapsed_ns = self.elapsed_ns.saturating_add(started.elapsed().as_nanos());
                result
            }
            Some(Metric::Allocation) => self.measure_allocations(work),
        }
    }

    #[cfg(feature = "allocation-metrics")]
    fn measure_allocations<T>(
        &mut self,
        work: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let mut result = None;
        let measured = allocation_counter::measure(|| result = Some(work()));
        self.allocations.count_total = self
            .allocations
            .count_total
            .saturating_add(measured.count_total);
        self.allocations.count_current = self
            .allocations
            .count_current
            .saturating_add(measured.count_current);
        self.allocations.count_peak = self.allocations.count_peak.max(measured.count_max);
        self.allocations.bytes_total = self
            .allocations
            .bytes_total
            .saturating_add(measured.bytes_total);
        self.allocations.bytes_current = self
            .allocations
            .bytes_current
            .saturating_add(measured.bytes_current);
        self.allocations.bytes_peak = self.allocations.bytes_peak.max(measured.bytes_max);
        result.ok_or_else(|| "segmented allocation result invariant failed".to_owned())?
    }

    #[cfg(not(feature = "allocation-metrics"))]
    fn measure_allocations<T>(
        &mut self,
        _work: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let _ = self.metric;
        Err("allocation capture requires the `allocation-metrics` feature".to_owned())
    }

    const fn finish(self) -> CapturedMetric {
        match self.metric {
            None => CapturedMetric::Warmup,
            Some(Metric::Timing) => CapturedMetric::Timing {
                elapsed_ns: self.elapsed_ns,
            },
            Some(Metric::Allocation) => CapturedMetric::Allocation(self.allocations),
        }
    }
}
