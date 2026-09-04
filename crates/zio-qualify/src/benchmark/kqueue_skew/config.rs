//! Minimal argument parsing and the review-defined skew matrix.

use std::{ffi::OsString, path::PathBuf};

pub(super) const MATRIX: [Row; 5] = [
    Row::new(100_000, 64, 100, "0.1_percent"),
    Row::new(100_000, 256, 1_000, "1_percent"),
    Row::new(100_000, 256, 50_000, "50_percent"),
    Row::new(100_000, 1_024, 100_000, "100_percent"),
    Row::new(1_000_000, 1_024, 1_024, "sparse_1024"),
];
const SMOKE: [Row; 1] = [Row::new(5, 2, 3, "smoke")];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Row {
    pub(super) registrations: usize,
    pub(super) event_capacity: usize,
    pub(super) ready: usize,
    pub(super) ready_label: &'static str,
}

impl Row {
    pub(super) const fn new(
        registrations: usize,
        event_capacity: usize,
        ready: usize,
        ready_label: &'static str,
    ) -> Self {
        Self {
            registrations,
            event_capacity,
            ready,
            ready_label,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Config {
    pub(super) output: Option<PathBuf>,
    pub(super) help: bool,
    smoke: bool,
}

impl Config {
    pub(super) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut output = None;
        let mut help = false;
        let mut smoke = false;
        let mut args = args.into_iter();
        while let Some(raw) = args.next() {
            let argument = raw
                .into_string()
                .map_err(|_| "arguments other than --output must be valid UTF-8".to_owned())?;
            match argument.as_str() {
                "--output" if output.is_none() => {
                    output = Some(PathBuf::from(
                        args.next()
                            .ok_or_else(|| "--output requires a path".to_owned())?,
                    ));
                }
                "--output" => return Err("--output may be supplied only once".to_owned()),
                "--smoke" if !smoke => smoke = true,
                "--smoke" => return Err("--smoke may be supplied only once".to_owned()),
                "--help" | "-h" if !help => help = true,
                "--help" | "-h" => return Err("--help may be supplied only once".to_owned()),
                _ => return Err(format!("unknown argument `{argument}`; use --help")),
            }
        }
        Ok(Self {
            output,
            help,
            smoke,
        })
    }

    pub(super) const fn rows(&self) -> &'static [Row] {
        if self.smoke { &SMOKE } else { &MATRIX }
    }
}

pub(super) const HELP: &str = "\
Usage: zio-kqueue-skew [--output PATH] [--smoke]\n\
\n\
Runs the fixed registration-to-event-capacity matrix from docs/performance.md.\n\
Unsupported targets and inadequate descriptor limits produce structured receipts.\n";
