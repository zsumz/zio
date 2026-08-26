//! Allocation sample serialization and exact normalization ratios.

use super::{
    json,
    measure::{Allocations, CapturedMetric},
    record::Sample,
};

pub(crate) fn raw(output: &mut String, samples: &[Sample]) -> Result<(), String> {
    let values = values(samples)?;
    for (name, select) in [
        ("allocation_count_total", Field::CountTotal),
        ("allocation_count_peak", Field::CountPeak),
        ("allocation_bytes_total", Field::BytesTotal),
        ("allocation_bytes_peak", Field::BytesPeak),
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

pub(crate) fn summary(output: &mut String, samples: &[Sample]) -> Result<(), String> {
    let values = values(samples)?;
    let count = sum_alloc(&values, true);
    let bytes = sum_alloc(&values, false);
    json::field_number(output, "allocation_count_total", count, true);
    json::field_number(output, "allocation_bytes_total", bytes, true);
    json::key(output, "allocation_rate");
    output.push('{');
    json::field_number(output, "count_numerator", count, true);
    json::field_number(output, "bytes_numerator", bytes, true);
    json::field_number(
        output,
        "operations_denominator",
        sum_counts(samples, true),
        true,
    );
    json::field_number(
        output,
        "events_denominator",
        sum_counts(samples, false),
        false,
    );
    output.push('}');
    Ok(())
}

fn values(samples: &[Sample]) -> Result<Vec<Allocations>, String> {
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
enum Field {
    CountTotal,
    CountPeak,
    BytesTotal,
    BytesPeak,
}

impl Field {
    const fn unsigned(self, value: Allocations) -> u64 {
        match self {
            Self::CountTotal => value.count_total,
            Self::CountPeak => value.count_peak,
            Self::BytesTotal => value.bytes_total,
            Self::BytesPeak => value.bytes_peak,
        }
    }
}
