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
    pub(super) run_id: String,
    pub(super) help: bool,
    smoke: bool,
}

impl Config {
    pub(super) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut output = None;
        let mut run_id = None;
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
                "--run-id" if run_id.is_none() => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--run-id requires a value".to_owned())?
                        .into_string()
                        .map_err(|_| "--run-id must be valid UTF-8".to_owned())?;
                    if value.is_empty()
                        || value.len() > 128
                        || !value
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
                    {
                        return Err(
                            "--run-id must contain 1 through 128 ASCII letters, digits, hyphens, or underscores"
                                .to_owned(),
                        );
                    }
                    run_id = Some(value);
                }
                "--run-id" => return Err("--run-id may be supplied only once".to_owned()),
                "--smoke" if !smoke => smoke = true,
                "--smoke" => return Err("--smoke may be supplied only once".to_owned()),
                "--help" | "-h" if !help => help = true,
                "--help" | "-h" => return Err("--help may be supplied only once".to_owned()),
                _ => return Err(format!("unknown argument `{argument}`; use --help")),
            }
        }
        let run_id = match run_id {
            Some(run_id) => run_id,
            None if smoke || help => "smoke-unbound".to_owned(),
            None => return Err("--run-id is required for the full matrix".to_owned()),
        };
        if !smoke && !help && !is_uuid(&run_id) {
            return Err("--run-id must be one lowercase hyphenated UUID".to_owned());
        }
        Ok(Self {
            output,
            run_id,
            help,
            smoke,
        })
    }

    pub(super) const fn rows(&self) -> &'static [Row] {
        if self.smoke { &SMOKE } else { &MATRIX }
    }
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
            }
        })
}

pub(super) const HELP: &str = "\
Usage: zio-kqueue-skew [--output PATH] [--run-id ID] [--smoke]\n\
\n\
Runs the fixed registration-to-event-capacity matrix from docs/performance.md.\n\
Full-matrix evidence requires a generated run UUID through --run-id.\n\
Unsupported targets and inadequate descriptor limits produce structured receipts.\n";
