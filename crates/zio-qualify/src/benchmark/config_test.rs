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
    let config = Config::parse([OsString::from("--smoke")], Metric::Timing)?;
    check(config.samples == 2, "smoke samples")?;
    check(
        config.iterations_for(Scenario::ReadyBatch1024) == 1,
        "smoke iterations",
    )?;
    check(
        Config::parse(
            [
                OsString::from("--smoke"),
                OsString::from("--samples"),
                OsString::from("1"),
            ],
            Metric::Timing,
        )
        .is_err(),
        "smoke accepted tuning",
    )
}

#[test]
fn defaults_are_scenario_aware_and_explicit_is_exact() -> Result<(), String> {
    let default = Config::parse([], Metric::Timing)?;
    check(default.samples == 96, "publication timing rounds")?;
    check(
        default.samples % Implementation::ALL.len() == 0 && default.samples % 3 == 0,
        "three- and four-candidate rotation balance",
    )?;
    check(
        default.iterations_for(Scenario::EmptyWait) == 100,
        "small default",
    )?;
    check(
        default.iterations_for(Scenario::ReadyBatch1024) == 4,
        "batch default",
    )?;
    let allocation = Config::parse([], Metric::Allocation)?;
    check(allocation.samples == 12, "allocation rounds")?;
    let explicit = Config::parse(
        [OsString::from("--iterations"), OsString::from("17")],
        Metric::Timing,
    )?;
    check(
        explicit.iterations_for(Scenario::ReadyBatch1024) == 17,
        "explicit iterations",
    )
}

#[test]
fn help_names_every_stable_scenario_for_each_binary() -> Result<(), String> {
    for metric in [Metric::Timing, Metric::Allocation] {
        let help = help(metric);
        check(
            help.contains("qualification for zio owned, zio borrowed, Mio, and polling"),
            "candidate summary missing from help",
        )?;
        for scenario in Scenario::ALL {
            check(help.contains(scenario.name()), "scenario missing from help")?;
        }
    }
    Ok(())
}

#[test]
fn rejects_unknown_zero_and_unexposed_pairs() -> Result<(), String> {
    let borrowed = Config::parse(
        [
            OsString::from("--implementation"),
            OsString::from("zio-borrowed"),
        ],
        Metric::Timing,
    )?;
    check(
        borrowed.implementation == Some(Implementation::ZioBorrowed),
        "borrowed candidate",
    )?;
    check(
        Config::parse(
            [OsString::from("--sample-time-ms"), OsString::from("10")],
            Metric::Allocation,
        )
        .is_err(),
        "allocation sample-time accepted",
    )?;
    check(
        Config::parse([OsString::from("--wat")], Metric::Timing).is_err(),
        "unknown",
    )?;
    check(
        Config::parse(
            [
                OsString::from("--samples"),
                OsString::from("1"),
                OsString::from("--samples"),
                OsString::from("2"),
            ],
            Metric::Timing,
        )
        .is_err(),
        "duplicate",
    )?;
    check(
        Config::parse(
            [OsString::from("--iterations"), OsString::from("0")],
            Metric::Timing,
        )
        .is_err(),
        "zero",
    )?;
    check(
        Config::parse(
            [
                OsString::from("--implementation"),
                OsString::from("mio"),
                OsString::from("--scenario"),
                OsString::from("wait.ready.readable.level.repeat"),
            ],
            Metric::Timing,
        )
        .is_err(),
        "Mio level label",
    )
}

fn check(condition: bool, message: &'static str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_owned())
}
