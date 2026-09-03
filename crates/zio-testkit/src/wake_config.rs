//! Waker key and clone conformance cases.

use core::time::Duration;

use zio::{Error, Key, Wait, Waker};

use crate::wake_verify::{
    events, expect_empty, expect_single_wake, mismatch, poll, reject_recovery, trigger, wait_for,
    waker,
};
use crate::{WakeCheck, WakeFailure, WakeScenario};

const SAME_KEY: Key = Key::new(4_001);
const EXISTING_KEY: Key = Key::new(4_002);
const REQUESTED_KEY: Key = Key::new(4_003);
const OBSERVATION_LIMIT: Duration = Duration::from_secs(1);

pub(crate) fn same_key_clones(scenario: WakeScenario) -> Result<(), WakeFailure> {
    let mut poll = poll(scenario, 1, 1)?;
    let original = waker(&mut poll, SAME_KEY, scenario)?;
    let repeated = waker(&mut poll, SAME_KEY, scenario)?;
    let cloned = original.clone();
    let mut events = events(&poll, scenario)?;
    let keys = [original.key(), repeated.key(), cloned.key()];

    if keys != [SAME_KEY; 3] {
        return mismatch(scenario, WakeCheck::Configuration, [SAME_KEY; 3], keys);
    }

    for candidate in [&original, &repeated, &cloned] {
        wake_and_drain(&mut poll, &mut events, candidate, SAME_KEY, scenario)?;
    }
    Ok(())
}

pub(crate) fn conflicting_key(scenario: WakeScenario) -> Result<(), WakeFailure> {
    let mut poll = poll(scenario, 1, 1)?;
    let original = waker(&mut poll, EXISTING_KEY, scenario)?;
    match poll.waker(REQUESTED_KEY) {
        Err(Error::WakerAlreadyConfigured {
            existing,
            requested,
        }) if (existing, requested) == (EXISTING_KEY, REQUESTED_KEY) => {}
        Err(error) => {
            return mismatch(
                scenario,
                WakeCheck::Configuration,
                Error::WakerAlreadyConfigured {
                    existing: EXISTING_KEY,
                    requested: REQUESTED_KEY,
                },
                error,
            );
        }
        Ok(_) => {
            return mismatch(
                scenario,
                WakeCheck::Configuration,
                "conflicting-key rejection",
                "configured conflicting waker",
            );
        }
    }

    let mut events = events(&poll, scenario)?;
    wake_and_drain(&mut poll, &mut events, &original, EXISTING_KEY, scenario)?;
    wake_and_drain(&mut poll, &mut events, &original, EXISTING_KEY, scenario)
}

fn wake_and_drain(
    poll: &mut zio::Poll,
    events: &mut zio::Events,
    waker: &Waker,
    key: Key,
    scenario: WakeScenario,
) -> Result<(), WakeFailure> {
    trigger(waker, scenario)?;
    let report = wait_for(poll, events, Wait::For(OBSERVATION_LIMIT), scenario)?;
    expect_single_wake(events, key, scenario)?;
    reject_recovery(report, events, scenario)?;
    let report = wait_for(poll, events, Wait::NoBlock, scenario)?;
    expect_empty(events, scenario)?;
    reject_recovery(report, events, scenario)
}
