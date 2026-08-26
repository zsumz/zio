//! Metric-specific samples and metric-independent resource fields.

use super::{
    json,
    measure::{CapturedMetric, Metric, distribution},
    receipt_allocation,
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
        receipt_allocation::raw(output, samples)?;
    }
    output.push('}');
    Ok(())
}

fn timing_raw(output: &mut String, samples: &[Sample]) -> Result<(), String> {
    let elapsed = timing_values(samples)?;
    output.push(',');
    json::key(output, "measured_elapsed_ns");
    json::array_u128(output, elapsed.iter().copied());
    output.push(',');
    json::key(output, "sample_mean_ns_per_operation");
    json::array_u128(
        output,
        samples.iter().zip(elapsed).map(|(sample, elapsed)| {
            elapsed / u128::from(sample.captured.counts.operations.max(1))
        }),
    );
    output.push(',');
    json::key(output, "sample_mean_ns_per_event");
    if samples
        .iter()
        .all(|sample| sample.captured.counts.events > 0)
    {
        let elapsed = timing_values(samples)?;
        json::array_u128(
            output,
            samples
                .iter()
                .zip(elapsed)
                .map(|(sample, elapsed)| elapsed / u128::from(sample.captured.counts.events)),
        );
    } else {
        output.push_str("null");
    }
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
        receipt_allocation::summary(output, samples)?;
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
    json::field_number(
        output,
        "median_sample_mean_ns_per_operation",
        summary.median,
        true,
    );
    json::field_number(
        output,
        "p95_sample_mean_ns_per_operation",
        summary.p95,
        true,
    );
    json::field_number(
        output,
        "mad_sample_mean_ns_per_operation",
        summary.mad,
        true,
    );
    json::field_number(
        output,
        "min_sample_mean_ns_per_operation",
        summary.minimum,
        true,
    );
    json::field_number(
        output,
        "max_sample_mean_ns_per_operation",
        summary.maximum,
        true,
    );
    let elapsed = distribution(&timing_values(samples)?)?;
    json::field_number(output, "median_measured_elapsed_ns", elapsed.median, true);
    json::key(output, "event_timing");
    if samples
        .iter()
        .all(|sample| sample.captured.counts.events > 0)
    {
        let values: Result<Vec<_>, _> = samples
            .iter()
            .map(|sample| {
                timing_value(sample)
                    .map(|elapsed| elapsed / u128::from(sample.captured.counts.events))
            })
            .collect();
        let event = distribution(&values?)?;
        output.push('{');
        json::field_number(
            output,
            "median_sample_mean_ns_per_event",
            event.median,
            true,
        );
        json::field_number(output, "p95_sample_mean_ns_per_event", event.p95, true);
        json::field_number(output, "mad_sample_mean_ns_per_event", event.mad, true);
        json::field_number(output, "min_sample_mean_ns_per_event", event.minimum, true);
        json::field_number(output, "max_sample_mean_ns_per_event", event.maximum, false);
        output.push('}');
    } else {
        output.push_str("null");
    }
    Ok(())
}

fn timing_values(samples: &[Sample]) -> Result<Vec<u128>, String> {
    samples.iter().map(timing_value).collect()
}

fn timing_value(sample: &Sample) -> Result<u128, String> {
    match sample.captured.metric {
        CapturedMetric::Timing { elapsed_ns } => Ok(elapsed_ns),
        _ => Err("timing receipt received a non-timing sample".to_owned()),
    }
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
