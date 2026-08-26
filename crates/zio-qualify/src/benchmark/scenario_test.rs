//! Scenario catalog and semantic-label tests.

use crate::Implementation;

use super::scenario::Scenario;

#[test]
fn scenario_names_are_stable() -> Result<(), String> {
    let expected = [
        "poller.construct_drop",
        "registration.register_delete",
        "wait.empty.no_block",
        "wait.ready.readable.single.initial",
        "wait.ready.readable.batch_64.initial",
        "wait.ready.readable.batch_1024.initial",
        "wake.notify.roundtrip",
        "wait.ready.readable.level.repeat",
        "wait.ready.readable.one_shot.disarm",
        "wait.ready.readable.one_shot.rearm",
    ];
    check(
        Scenario::ALL.map(Scenario::name) == expected,
        "scenario names",
    )
}

#[test]
fn mio_is_never_scheduled_under_exact_delivery_labels() -> Result<(), String> {
    for scenario in [
        Scenario::LevelRepeat,
        Scenario::OneShotDisarm,
        Scenario::OneShotRearm,
    ] {
        check(!scenario.supports(Implementation::Mio), "Mio exact profile")?;
    }
    for scenario in [
        Scenario::ReadySingle,
        Scenario::ReadyBatch64,
        Scenario::ReadyBatch1024,
    ] {
        check(
            scenario.candidate_setup(Implementation::Mio) == "mio_native_default",
            "Mio native-default label",
        )?;
    }
    Ok(())
}

#[test]
fn wait_and_absence_windows_match_the_measured_work() -> Result<(), String> {
    check(
        Scenario::ConstructDrop.wait_timeout_ms().is_none(),
        "construct wait",
    )?;
    check(
        Scenario::RegisterDelete.wait_timeout_ms().is_none(),
        "register wait",
    )?;
    check(
        Scenario::EmptyWait.wait_timeout_ms() == Some(0),
        "nonblocking wait",
    )?;
    for scenario in [
        Scenario::ReadySingle,
        Scenario::ReadyBatch64,
        Scenario::ReadyBatch1024,
        Scenario::WakeRoundtrip,
        Scenario::LevelRepeat,
        Scenario::OneShotDisarm,
        Scenario::OneShotRearm,
    ] {
        check(scenario.wait_timeout_ms() == Some(1_000), "bounded wait")?;
    }
    check(
        Scenario::OneShotDisarm.absence_window_ms() == Some(2)
            && Scenario::OneShotDisarm.absence_window_timed() == Some(true),
        "timed disarm absence",
    )?;
    check(
        Scenario::OneShotRearm.absence_window_ms() == Some(2)
            && Scenario::OneShotRearm.absence_window_timed() == Some(false),
        "untimed rearm absence",
    )?;
    check(
        Scenario::ReadySingle.absence_window_ms().is_none()
            && Scenario::ReadySingle.absence_window_timed().is_none(),
        "unrelated absence",
    )
}

fn check(condition: bool, message: &'static str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_owned())
}
