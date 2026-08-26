//! Metric-specific samples and metric-independent resource fields.

use super::{
    json,
    measure::{Allocations, CapturedMetric, FdProbe, Metric, distribution},
    record::Sample,
};

pub(crate) fn raw(output: &mut String, metric: Metric, samples: &[Sample]) -> Result<(), String> {
    json::key(output, "raw");
    output.push('{');
    json::key(output, "round");
    json::array_usize(output, samples.iter().map(|sample| sample.round));
    output.push(',');
    json::key(output, "order_position");
    json::array_usize(output, samples.iter().map(|sample| sample.order_position));
    output.push(',');
    json::key(output, "operations");
    json::array_u64(
        output,
        samples
            .iter()
            .map(|sample| sample.captured.counts.operations),
    );
    output.push(',');
    json::key(output, "events");
    json::array_u64(
        output,
        samples.iter().map(|sample| sample.captured.counts.events),
    );
    if metric == Metric::Timing {
        timing_raw(output, samples)?;
    } else {
        allocation_raw(output, samples)?;
    }
    output.push('}');
    Ok(())
}

fn timing_raw(output: &mut String, samples: &[Sample]) -> Result<(), String> {
    let elapsed = timing_values(samples)?;
    output.push(',');
    json::key(output, "elapsed_ns");
    json::array_u128(output, elapsed.iter().copied());
    output.push(',');
    json::key(output, "ns_per_operation");
    json::array_u128(
        output,
        samples.iter().zip(elapsed).map(|(sample, elapsed)| {
            elapsed / u128::from(sample.captured.counts.operations.max(1))
        }),
    );
    Ok(())
}

fn allocation_raw(output: &mut String, samples: &[Sample]) -> Result<(), String> {
    let values = allocation_values(samples)?;
    for (name, select) in [
        ("allocation_count_total", AllocationField::CountTotal),
        ("allocation_count_peak", AllocationField::CountPeak),
        ("allocation_bytes_total", AllocationField::BytesTotal),
        ("allocation_bytes_peak", AllocationField::BytesPeak),
    ] {
        output.push(',');
        json::key(output, name);
        json::array_u64(output, values.iter().map(|value| select.unsigned(*value)));
    }
    output.push(',');
    json::key(output, "allocation_count_current");
    json::array_i64(output, values.iter().map(|value| value.count_current));
    output.push(',');
    json::key(output, "allocation_bytes_current");
    json::array_i64(output, values.iter().map(|value| value.bytes_current));
    Ok(())
}

pub(crate) fn summary(
    output: &mut String,
    metric: Metric,
    samples: &[Sample],
) -> Result<(), String> {
    json::key(output, "summary");
    output.push('{');
    json::field_number(output, "operations", sum_counts(samples, true), true);
    json::field_number(output, "events", sum_counts(samples, false), true);
    if metric == Metric::Timing {
        timing_summary(output, samples)?;
    } else {
        allocation_summary(output, samples)?;
    }
    output.push('}');
    Ok(())
}

fn timing_summary(output: &mut String, samples: &[Sample]) -> Result<(), String> {
    let per_operation: Vec<_> = samples
        .iter()
        .zip(timing_values(samples)?)
        .map(|(sample, elapsed)| elapsed / u128::from(sample.captured.counts.operations.max(1)))
        .collect();
    let summary = distribution(&per_operation)?;
    json::field_number(output, "median_ns_per_operation", summary.median, true);
    json::field_number(output, "p95_ns_per_operation", summary.p95, true);
    json::field_number(output, "mad_ns_per_operation", summary.mad, true);
    json::field_number(output, "min_ns_per_operation", summary.minimum, true);
    json::field_number(output, "max_ns_per_operation", summary.maximum, false);
    Ok(())
}

fn allocation_summary(output: &mut String, samples: &[Sample]) -> Result<(), String> {
    let values = allocation_values(samples)?;
    json::field_number(
        output,
        "allocation_count_total",
        sum_alloc(&values, true),
        true,
    );
    json::field_number(
        output,
        "allocation_bytes_total",
        sum_alloc(&values, false),
        false,
    );
    Ok(())
}

pub(crate) fn retained_fds(output: &mut String, probe: &FdProbe, samples: &[Sample]) {
    json::key(output, "retained_fds");
    output.push('{');
    if samples.is_empty() {
        json::field_string(output, "status", "unavailable", true);
        json::field_string(output, "reason", "no completed samples", false);
    } else if samples
        .iter()
        .all(|sample| sample.retained_fd_delta.is_some())
    {
        json::field_string(output, "status", "available", true);
        json::field_string(output, "path", probe.path().unwrap_or("unavailable"), true);
        json::key(output, "raw_delta");
        json::array_i64(
            output,
            samples.iter().filter_map(|sample| sample.retained_fd_delta),
        );
    } else {
        json::field_string(output, "status", "unavailable", true);
        json::field_string(
            output,
            "reason",
            probe
                .reason()
                .unwrap_or("descriptor directory became unreadable during sampling"),
            false,
        );
    }
    output.push('}');
}

fn timing_values(samples: &[Sample]) -> Result<Vec<u128>, String> {
    samples
        .iter()
        .map(|sample| match sample.captured.metric {
            CapturedMetric::Timing { elapsed_ns } => Ok(elapsed_ns),
            _ => Err("timing receipt received a non-timing sample".to_owned()),
        })
        .collect()
}

fn allocation_values(samples: &[Sample]) -> Result<Vec<Allocations>, String> {
    samples
        .iter()
        .map(|sample| match sample.captured.metric {
            CapturedMetric::Allocation(value) => Ok(value),
            _ => Err("allocation receipt received a non-allocation sample".to_owned()),
        })
        .collect()
}

fn sum_counts(samples: &[Sample], operations: bool) -> u64 {
    samples.iter().fold(0, |sum, sample| {
        sum.saturating_add(if operations {
            sample.captured.counts.operations
        } else {
            sample.captured.counts.events
        })
    })
}

fn sum_alloc(values: &[Allocations], count: bool) -> u64 {
    values.iter().fold(0_u64, |sum, value| {
        sum.saturating_add(if count {
            value.count_total
        } else {
            value.bytes_total
        })
    })
}

#[derive(Clone, Copy)]
enum AllocationField {
    CountTotal,
    CountPeak,
    BytesTotal,
    BytesPeak,
}

impl AllocationField {
    const fn unsigned(self, value: Allocations) -> u64 {
        match self {
            Self::CountTotal => value.count_total,
            Self::CountPeak => value.count_peak,
            Self::BytesTotal => value.bytes_total,
            Self::BytesPeak => value.bytes_peak,
        }
    }
}
