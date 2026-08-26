//! Strict command-line parsing and benchmark defaults.

use std::{ffi::OsString, path::PathBuf};

use crate::Implementation;

use super::{
    config_parse::{
        implementation as parse_implementation, number, number_u64, set_flag, set_value, text,
    },
    measure::Metric,
    scenario::Scenario,
};

pub(crate) use super::config_help::help;

const MAX_SAMPLES: usize = 1_000;
const MAX_ITERATIONS: usize = 1_000_000;
const MAX_WARMUP: usize = 100_000;
const MAX_SAMPLE_TIME_MS: u64 = 10_000;
pub(crate) const DEFAULT_SAMPLE_TIME_MS: u64 = 100;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Config {
    pub(crate) samples: usize,
    pub(crate) iterations: Option<usize>,
    pub(crate) warmup_iterations: usize,
    pub(crate) warmup_explicit: bool,
    pub(crate) target_sample_time_ms: u64,
    pub(crate) implementation: Option<Implementation>,
    pub(crate) scenario: Option<Scenario>,
    pub(crate) output: Option<PathBuf>,
    pub(crate) help: bool,
    pub(crate) smoke: bool,
}

impl Config {
    pub(crate) fn parse(
        args: impl IntoIterator<Item = OsString>,
        metric: Metric,
    ) -> Result<Self, String> {
        let mut builder = Builder::default();
        let mut args = args.into_iter();
        while let Some(raw) = args.next() {
            let argument = raw
                .into_string()
                .map_err(|_| "arguments other than --output must be valid UTF-8".to_owned())?;
            match argument.as_str() {
                "--samples" => set_value(
                    &mut builder.samples,
                    number(&mut args, "--samples", MAX_SAMPLES)?,
                    "--samples",
                )?,
                "--iterations" => {
                    set_value(
                        &mut builder.iterations,
                        number(&mut args, "--iterations", MAX_ITERATIONS)?,
                        "--iterations",
                    )?;
                }
                "--warmup" => set_value(
                    &mut builder.warmup,
                    number(&mut args, "--warmup", MAX_WARMUP)?,
                    "--warmup",
                )?,
                "--sample-time-ms" => set_value(
                    &mut builder.sample_time_ms,
                    number_u64(&mut args, "--sample-time-ms", MAX_SAMPLE_TIME_MS)?,
                    "--sample-time-ms",
                )?,
                "--implementation" => {
                    let value = text(&mut args, "--implementation")?;
                    set_value(
                        &mut builder.implementation,
                        parse_implementation(&value)?,
                        "--implementation",
                    )?;
                }
                "--scenario" => {
                    let value = text(&mut args, "--scenario")?;
                    set_value(
                        &mut builder.scenario,
                        Scenario::parse(&value).ok_or_else(|| {
                            format!("unknown scenario `{value}`; use --help for stable names")
                        })?,
                        "--scenario",
                    )?;
                }
                "--output" => {
                    let path = PathBuf::from(
                        args.next()
                            .ok_or_else(|| "--output requires a path".to_owned())?,
                    );
                    set_value(&mut builder.output, path, "--output")?;
                }
                "--smoke" => set_flag(&mut builder.smoke, "--smoke")?,
                "--help" | "-h" => set_flag(&mut builder.help, "--help")?,
                _ => return Err(format!("unknown argument `{argument}`; use --help")),
            }
        }
        builder.finish(metric)
    }

    pub(crate) const fn iterations_for(&self, scenario: Scenario) -> usize {
        if self.smoke {
            1
        } else if let Some(iterations) = self.iterations {
            iterations
        } else {
            scenario.default_iterations()
        }
    }

    pub(crate) fn iterations_source(&self, metric: Metric, _scenario: Scenario) -> &'static str {
        if self.smoke {
            "smoke"
        } else if self.iterations.is_some() {
            "explicit"
        } else if metric == Metric::Timing {
            "calibrated_shared"
        } else {
            "scenario_default"
        }
    }

    pub(crate) fn calibrates(&self, metric: Metric, _scenario: Scenario) -> bool {
        metric == Metric::Timing && !self.smoke && self.iterations.is_none()
    }

    pub(crate) fn warmup_for(
        &self,
        metric: Metric,
        scenario: Scenario,
        shared_iterations: usize,
    ) -> usize {
        if self.smoke || self.warmup_explicit || !self.calibrates(metric, scenario) {
            self.warmup_iterations
        } else {
            shared_iterations.saturating_mul(3)
        }
    }

    pub(crate) fn warmup_source(&self, metric: Metric, scenario: Scenario) -> &'static str {
        if self.smoke {
            "smoke"
        } else if self.warmup_explicit {
            "explicit"
        } else if self.calibrates(metric, scenario) {
            "three_shared_iteration_batches"
        } else {
            "scenario_default"
        }
    }
}

#[derive(Default)]
struct Builder {
    samples: Option<usize>,
    iterations: Option<usize>,
    warmup: Option<usize>,
    sample_time_ms: Option<u64>,
    implementation: Option<Implementation>,
    scenario: Option<Scenario>,
    output: Option<PathBuf>,
    smoke: bool,
    help: bool,
}

impl Builder {
    fn finish(self, metric: Metric) -> Result<Config, String> {
        if metric == Metric::Allocation && self.sample_time_ms.is_some() {
            return Err("--sample-time-ms is available only for timing receipts".to_owned());
        }
        if self.smoke
            && (self.samples.is_some()
                || self.iterations.is_some()
                || self.warmup.is_some()
                || self.sample_time_ms.is_some())
        {
            return Err("--smoke cannot be combined with sampling or warmup tuning".to_owned());
        }
        let warmup_explicit = self.warmup.is_some();
        let (samples, iterations, warmup_iterations) = if self.smoke {
            (2, None, 1)
        } else {
            (
                self.samples.unwrap_or(match metric {
                    Metric::Timing => 90,
                    Metric::Allocation => 12,
                }),
                self.iterations,
                self.warmup.unwrap_or(10),
            )
        };
        if let (Some(implementation), Some(scenario)) = (self.implementation, self.scenario)
            && !scenario.supports(implementation)
        {
            return Err(format!(
                "scenario `{}` is not exposed by `{}`",
                scenario.name(),
                implementation.name()
            ));
        }
        Ok(Config {
            samples,
            iterations,
            warmup_iterations,
            warmup_explicit,
            target_sample_time_ms: self.sample_time_ms.unwrap_or(DEFAULT_SAMPLE_TIME_MS),
            implementation: self.implementation,
            scenario: self.scenario,
            output: self.output,
            help: self.help,
            smoke: self.smoke,
        })
    }
}
