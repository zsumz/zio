//! Private benchmark runner behind the `zio-perf` binary.

mod backend;
mod candidate_bench;
mod config;
#[cfg(test)]
mod config_test;
mod json;
#[cfg(test)]
mod json_test;
mod measure;
#[cfg(test)]
mod measure_test;
mod metadata;
mod mio_backend;
mod polling_backend;
mod polling_direct;
mod profile_workload;
mod ready_workload;
mod receipt;
mod receipt_context;
mod receipt_data;
#[cfg(test)]
mod receipt_test;
mod record;
mod resource_limit;
#[cfg(test)]
mod resource_limit_test;
mod runner;
mod runner_support;
#[cfg(test)]
mod runner_test;
mod scenario;
#[cfg(test)]
mod scenario_test;
mod workload;
mod zio_backend;

use std::{ffi::OsString, fs::File, io, path::Path};

use measure::Metric;

/// Runs the private performance qualification command.
pub fn run_perf(args: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    if cfg!(feature = "allocation-metrics") {
        return Err(
            "timing receipts require a build without the `allocation-metrics` feature".to_owned(),
        );
    }
    run(args, Metric::Timing)
}

/// Runs the allocation-only private resource qualification command.
#[cfg(feature = "allocation-metrics")]
pub fn run_perf_alloc(args: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    run(args, Metric::Allocation)
}

fn run(args: impl IntoIterator<Item = OsString>, metric: Metric) -> Result<(), String> {
    let config = config::Config::parse(args)?;
    if config.help {
        print!("{}", config::help(metric));
        return Ok(());
    }
    let metadata = metadata::Metadata::collect();
    let mut stderr = io::stderr().lock();
    match config.output.as_deref() {
        None => runner::execute(
            metric,
            &config,
            &metadata,
            &mut io::stdout().lock(),
            &mut stderr,
        ),
        Some(path) if path == Path::new("-") => runner::execute(
            metric,
            &config,
            &metadata,
            &mut io::stdout().lock(),
            &mut stderr,
        ),
        Some(path) => {
            let mut output = File::create(path).map_err(display)?;
            runner::execute(metric, &config, &metadata, &mut output, &mut stderr)
        }
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
