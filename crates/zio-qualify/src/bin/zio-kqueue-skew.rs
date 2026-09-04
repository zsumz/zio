//! Kqueue registration-to-delivery skew qualification.

use std::process::ExitCode;

fn main() -> ExitCode {
    match zio_qualify::run_kqueue_skew(std::env::args_os().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("zio-kqueue-skew: {error}");
            ExitCode::FAILURE
        }
    }
}
