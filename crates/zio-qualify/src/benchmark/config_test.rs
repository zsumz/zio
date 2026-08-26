//! Command-line contract tests.

use std::ffi::OsString;

use crate::Implementation;

use super::{
    config::{Config, help},
    measure::Metric,
    scenario::Scenario,
};

#[test]
fn smoke_is_bounded_and_rejects_tuning() -> Result<(), String> {
    let config = Config::parse([OsString::from("--smoke")])?;
    check(config.samples == 2, "smoke samples")?;
    check(
        config.iterations_for(Scenario::ReadyBatch1024) == 1,
        "smoke iterations",
    )?;
    check(
        Config::parse([
            OsString::from("--smoke"),
            OsString::from("--samples"),
            OsString::from("1"),
        ])
        .is_err(),
        "smoke accepted tuning",
    )
}

#[test]
fn defaults_are_scenario_aware_and_explicit_is_exact() -> Result<(), String> {
    let default = Config::parse([])?;
    check(default.samples == 12, "balanced measured rounds")?;
    check(
        default.samples % Implementation::ALL.len() == 0 && default.samples % 2 == 0,
        "two- and three-candidate rotation balance",
    )?;
    check(
        default.iterations_for(Scenario::EmptyWait) == 100,
        "small default",
    )?;
    check(
        default.iterations_for(Scenario::ReadyBatch1024) == 4,
        "batch default",
    )?;
    let explicit = Config::parse([OsString::from("--iterations"), OsString::from("17")])?;
    check(
        explicit.iterations_for(Scenario::ReadyBatch1024) == 17,
        "explicit iterations",
    )
}

#[test]
fn help_names_every_stable_scenario_for_each_binary() -> Result<(), String> {
    for metric in [Metric::Timing, Metric::Allocation] {
        let help = help(metric);
        for scenario in Scenario::ALL {
            check(help.contains(scenario.name()), "scenario missing from help")?;
        }
    }
    Ok(())
}

#[test]
fn rejects_unknown_zero_and_unexposed_pairs() -> Result<(), String> {
    check(Config::parse([OsString::from("--wat")]).is_err(), "unknown")?;
    check(
        Config::parse([
            OsString::from("--samples"),
            OsString::from("1"),
            OsString::from("--samples"),
            OsString::from("2"),
        ])
        .is_err(),
        "duplicate",
    )?;
    check(
        Config::parse([OsString::from("--iterations"), OsString::from("0")]).is_err(),
        "zero",
    )?;
    check(
        Config::parse([
            OsString::from("--implementation"),
            OsString::from("mio"),
            OsString::from("--scenario"),
            OsString::from("wait.ready.readable.level.repeat"),
        ])
        .is_err(),
        "Mio level label",
    )
}

fn check(condition: bool, message: &'static str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_owned())
}
