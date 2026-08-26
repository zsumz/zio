//! Allocation-only private resource qualification.

use std::process::ExitCode;

fn main() -> ExitCode {
    match zio_qualify::run_perf_alloc(std::env::args_os().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("zio-perf-alloc: {error}");
            ExitCode::FAILURE
        }
    }
}
