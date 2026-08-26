//! Strict command-line parsing and benchmark defaults.

use std::{ffi::OsString, path::PathBuf};

use crate::Implementation;

use super::{measure::Metric, scenario::Scenario};

const MAX_SAMPLES: usize = 1_000;
const MAX_ITERATIONS: usize = 1_000_000;
const MAX_WARMUP: usize = 100_000;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Config {
    pub(crate) samples: usize,
    pub(crate) iterations: Option<usize>,
    pub(crate) warmup_iterations: usize,
    pub(crate) implementation: Option<Implementation>,
    pub(crate) scenario: Option<Scenario>,
    pub(crate) output: Option<PathBuf>,
    pub(crate) help: bool,
    pub(crate) smoke: bool,
}

impl Config {
    pub(crate) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
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
        builder.finish()
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

    pub(crate) const fn iterations_source(&self) -> &'static str {
        if self.smoke {
            "smoke"
        } else if self.iterations.is_some() {
            "explicit"
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
    implementation: Option<Implementation>,
    scenario: Option<Scenario>,
    output: Option<PathBuf>,
    smoke: bool,
    help: bool,
}

impl Builder {
    fn finish(self) -> Result<Config, String> {
        if self.smoke
            && (self.samples.is_some() || self.iterations.is_some() || self.warmup.is_some())
        {
            return Err(
                "--smoke cannot be combined with --samples, --iterations, or --warmup".to_owned(),
            );
        }
        let (samples, iterations, warmup_iterations) = if self.smoke {
            (2, None, 1)
        } else {
            (
                self.samples.unwrap_or(12),
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
            implementation: self.implementation,
            scenario: self.scenario,
            output: self.output,
            help: self.help,
            smoke: self.smoke,
        })
    }
}

fn number(
    args: &mut impl Iterator<Item = OsString>,
    flag: &'static str,
    maximum: usize,
) -> Result<usize, String> {
    let value = text(args, flag)?;
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{flag} requires an integer from 1 through {maximum}"))?;
    if parsed == 0 || parsed > maximum {
        return Err(format!(
            "{flag} must be between 1 and {maximum}; received {parsed}"
        ));
    }
    Ok(parsed)
}

fn text(args: &mut impl Iterator<Item = OsString>, flag: &'static str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value"))?
        .into_string()
        .map_err(|_| format!("{flag} requires a valid UTF-8 value"))
}

fn parse_implementation(value: &str) -> Result<Implementation, String> {
    match value {
        "zio" => Ok(Implementation::Zio),
        "mio" => Ok(Implementation::Mio),
        "polling" => Ok(Implementation::Polling),
        _ => Err(format!(
            "unknown implementation `{value}`; expected zio, mio, or polling"
        )),
    }
}

fn set_flag(slot: &mut bool, flag: &'static str) -> Result<(), String> {
    if *slot {
        Err(format!("duplicate {flag}"))
    } else {
        *slot = true;
        Ok(())
    }
}

fn set_value<T>(slot: &mut Option<T>, value: T, flag: &'static str) -> Result<(), String> {
    if slot.is_some() {
        Err(format!("duplicate {flag}"))
    } else {
        *slot = Some(value);
        Ok(())
    }
}

pub(crate) fn help(metric: Metric) -> String {
    format!(
        "{command}: reproducible Zio, Mio, and polling {metric} qualification\n\
\n\
USAGE: {command} [OPTIONS]\n\
  --samples N                 measured rounds (1..=1000; default 12)\n\
  --iterations N              exact iterations per sample (1..=1000000)\n\
  --warmup N                  unmeasured iterations (1..=100000; default 10)\n\
  --implementation NAME       zio | mio | polling\n\
  --scenario NAME             one stable scenario name\n\
  --output PATH               NDJSON path; '-' or omitted writes stdout\n\
  --smoke                     2 samples, 1 iteration, 1 warmup\n\
  --help                      show this help\n\
\n\
STABLE SCENARIOS:\n\
  poller.construct_drop\n\
  registration.register_delete\n\
  wait.empty.no_block\n\
  wait.ready.readable.single.initial\n\
  wait.ready.readable.batch_64.initial\n\
  wait.ready.readable.batch_1024.initial\n\
  wake.notify.roundtrip\n\
  wait.ready.readable.level.repeat\n\
  wait.ready.readable.one_shot.disarm\n\
  wait.ready.readable.one_shot.rearm\n",
        command = match metric {
            Metric::Timing => "zio-perf",
            Metric::Allocation => "zio-perf-alloc",
        },
        metric = metric.name(),
    )
}
