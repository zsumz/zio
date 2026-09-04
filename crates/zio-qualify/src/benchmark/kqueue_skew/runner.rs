//! Command orchestration and per-row execution.

use std::{
    ffi::OsString,
    fs::File,
    io::{self, Write},
    path::Path,
};

use super::{
    config::{Config, HELP, Row},
    model::Outcome,
    receipt, resource,
};
use crate::benchmark::metadata::Metadata;

/// Runs the fixed kqueue skew matrix and writes one receipt per row.
pub fn run(args: impl IntoIterator<Item = OsString>) -> Result<(), String> {
    let config = Config::parse(args)?;
    if config.help {
        print!("{HELP}");
        return Ok(());
    }
    let metadata = Metadata::collect();
    match config.output.as_deref() {
        None => execute(config.rows(), &metadata, &mut io::stdout().lock()),
        Some(path) if path == Path::new("-") => {
            execute(config.rows(), &metadata, &mut io::stdout().lock())
        }
        Some(path) => {
            let mut output = File::create(path).map_err(display)?;
            execute(config.rows(), &metadata, &mut output)
        }
    }
}

fn execute(rows: &[Row], metadata: &Metadata, output: &mut impl Write) -> Result<(), String> {
    let mut failures = 0_usize;
    for &row in rows {
        let resources = resource::inspect(row)?;
        let outcome = match resource::unsupported(resources) {
            Some((code, reason)) => Outcome::Unsupported { code, reason },
            None => execute_row(row),
        };
        failures = failures.saturating_add(usize::from(matches!(outcome, Outcome::Failed(_))));
        writeln!(
            output,
            "{}",
            receipt::encode(metadata, row, resources, &outcome)
        )
        .map_err(display)?;
    }
    if failures == 0 {
        Ok(())
    } else {
        Err(format!(
            "{failures} kqueue skew row(s) failed; receipts retain the evidence"
        ))
    }
}

fn execute_row(row: Row) -> Outcome {
    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    {
        let measured = super::fixture::measure(row, zio::Mode::Level).and_then(|level| {
            super::fixture::measure(row, zio::Mode::OneShot)
                .map(|one_shot| Outcome::Passed { level, one_shot })
        });
        measured.unwrap_or_else(Outcome::Failed)
    }
    #[cfg(not(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd")))]
    {
        let _ = row;
        Outcome::Unsupported {
            code: "kqueue_unavailable",
            reason: "the host is not a supported kqueue target".to_owned(),
        }
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
