//! Scenario catalog and semantic-label tests.

use crate::Implementation;

use super::scenario::Scenario;

#[test]
fn scenario_names_are_stable() -> Result<(), String> {
    let expected = [
        "poller.construct_drop.capacity_1",
        "poller.construct_drop.capacity_64",
        "poller.construct_drop.capacity_1024",
        "poller.construct_waker_drop.capacity_1",
        "poller.construct_waker_drop.capacity_64",
        "poller.construct_waker_drop.capacity_1024",
        "registration.register_delete",
        "registration.register.batch_64",
        "registration.delete.batch_64",
        "wait.empty.no_block",
        "wait.ready.readable.single.initial",
        "wait.ready.readable.batch_64.initial",
        "wait.ready.readable.batch_1024.initial",
        "wait.ready.readable.single.persistent",
        "wait.ready.readable.batch_64.persistent",
        "wait.ready.readable.batch_1024.persistent",
        "wake.notify.pretriggered",
        "wake.notify.blocked_cross_thread",
        "wait.ready.readable.level.repeat",
        "wait.ready.readable.one_shot.rearm",
    ];
    check(
        Scenario::ALL.map(Scenario::name) == expected,
        "scenario names",
    )
}

#[test]
fn mio_is_never_scheduled_under_exact_delivery_labels() -> Result<(), String> {
    for scenario in [Scenario::LevelRepeat, Scenario::OneShotRearm] {
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
fn construction_setup_discloses_wake_normalization() -> Result<(), String> {
    check(
        Scenario::Construct1024.candidate_setup(Implementation::Zio)
            == "eager_native_wake_source_without_public_waker",
        "Zio eager wake disclosure",
    )?;
    check(
        Scenario::ConstructWaker1024.candidate_setup(Implementation::Mio)
            == "external_usable_wake_handle_materialized",
        "normalized waker disclosure",
    )
}

#[test]
fn wait_and_absence_windows_match_the_measured_work() -> Result<(), String> {
    check(
        Scenario::Construct1024.wait_timeout_ms().is_none(),
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
        Scenario::OneShotRearm,
    ] {
        check(scenario.wait_timeout_ms() == Some(1_000), "bounded wait")?;
    }
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
