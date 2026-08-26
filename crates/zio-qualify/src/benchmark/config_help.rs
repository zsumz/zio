//! Benchmark command help generated from the stable scenario catalog.

use super::{measure::Metric, scenario::Scenario};

pub(crate) fn help(metric: Metric) -> String {
    format!(
        "{command}: reproducible Zio, Mio, and polling {metric} qualification\n\
\n\
USAGE: {command} [OPTIONS]\n\
  --samples N                 measured rounds (default {samples})\n\
  --iterations N              exact iterations per sample (1..=1000000)\n\
  --warmup N                  exact unmeasured iterations (1..=100000)\n\
  --sample-time-ms N          timing-only calibrated target (1..=10000; default 100)\n\
  --implementation NAME       zio | mio | polling\n\
  --scenario NAME             one stable scenario name\n\
  --output PATH               NDJSON path; '-' or omitted writes stdout\n\
  --smoke                     2 samples, 1 iteration, 1 warmup\n\
  --help                      show this help\n\
\n\
STABLE SCENARIOS:\n{scenarios}",
        command = match metric {
            Metric::Timing => "zio-perf",
            Metric::Allocation => "zio-perf-alloc",
        },
        metric = metric.name(),
        samples = match metric {
            Metric::Timing => 90,
            Metric::Allocation => 12,
        },
        scenarios = Scenario::ALL
            .iter()
            .map(|scenario| format!("  {}", scenario.name()))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}
