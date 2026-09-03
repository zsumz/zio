//! Capacity-one resource and wake saturation conformance.

use core::time::Duration;

use zio::{ArmState, Event, Interest, Key, Mode, RegistrationState, Wait};

use crate::wake_verify::{
    events, expect_empty, mismatch, observed, poll, reject_recovery, trigger, wait_for, waker,
};
use crate::{WakeCheck, WakeFailure, WakeScenario};

const RESOURCE_KEY: Key = Key::new(4_201);
const WAKE_KEY: Key = Key::new(4_202);
const MAX_DRAINS: usize = 4;
const OBSERVATION_LIMIT: Duration = Duration::from_secs(1);

pub(crate) fn capacity_one(scenario: WakeScenario) -> Result<(), WakeFailure> {
    #[cfg(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "netbsd"
    ))]
    {
        let (source, mut peer) = std::os::unix::net::UnixStream::pair()
            .map_err(|error| observed(scenario, WakeCheck::Setup, "UnixStream pair", &error))?;
        source
            .set_nonblocking(true)
            .map_err(|error| observed(scenario, WakeCheck::Setup, "nonblocking source", &error))?;
        let mut poll = poll(scenario, 1, 1)?;
        let registration = poll
            .register(&source, RESOURCE_KEY, Interest::READABLE, Mode::OneShot)
            .map_err(|error| observed(scenario, WakeCheck::Setup, "registered resource", &error))?;
        let waker = waker(&mut poll, WAKE_KEY, scenario)?;
        let mut events = events(&poll, scenario)?;

        std::io::Write::write_all(&mut peer, b"ready")
            .map_err(|error| observed(scenario, WakeCheck::Setup, "ready resource", &error))?;
        trigger(&waker, scenario)?;

        let mut saw_resource = false;
        let mut saw_wake = false;
        for attempt in 0..MAX_DRAINS {
            let request = if attempt == 0 {
                Wait::For(OBSERVATION_LIMIT)
            } else {
                Wait::NoBlock
            };
            let report = wait_for(&mut poll, &mut events, request, scenario)?;
            if events.len() > 1 {
                return mismatch(
                    scenario,
                    WakeCheck::Events,
                    "at most one event at capacity one",
                    events.as_slice(),
                );
            }
            if let Some(event) = events.get(0).copied() {
                observe_event(event, &mut saw_resource, &mut saw_wake, scenario)?;
            }
            reject_recovery(report, &events, scenario)?;
            if saw_resource && saw_wake {
                break;
            }
        }
        if !(saw_resource && saw_wake) {
            return mismatch(
                scenario,
                WakeCheck::Events,
                "one resource event and one wake within bounded drains",
                (saw_resource, saw_wake),
            );
        }

        match poll.registration_state(&registration) {
            Ok(RegistrationState::Registered {
                arm: ArmState::Disarmed,
            }) => {}
            actual => {
                return mismatch(
                    scenario,
                    WakeCheck::State,
                    RegistrationState::Registered {
                        arm: ArmState::Disarmed,
                    },
                    actual,
                );
            }
        }
        let report = wait_for(&mut poll, &mut events, Wait::NoBlock, scenario)?;
        expect_empty(&events, scenario)?;
        reject_recovery(report, &events, scenario)?;
        poll.delete(registration)
            .map_err(|error| observed(scenario, WakeCheck::Cleanup, "deleted registration", &error))
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "netbsd"
    )))]
    {
        Err(WakeFailure::new(
            scenario,
            WakeCheck::Setup,
            "supported zio readiness backend",
            "unsupported target",
        ))
    }
}

fn observe_event(
    event: Event,
    saw_resource: &mut bool,
    saw_wake: &mut bool,
    scenario: WakeScenario,
) -> Result<(), WakeFailure> {
    match event {
        Event::Resource { key, readiness, .. }
            if key == RESOURCE_KEY && readiness.contains(zio::Readiness::READABLE) =>
        {
            if *saw_resource {
                return mismatch(
                    scenario,
                    WakeCheck::Events,
                    "one resource delivery",
                    "duplicate resource delivery",
                );
            }
            *saw_resource = true;
            Ok(())
        }
        Event::Wake { key, .. } if key == WAKE_KEY => {
            if *saw_wake {
                return mismatch(
                    scenario,
                    WakeCheck::Events,
                    "one wake delivery",
                    "duplicate wake delivery",
                );
            }
            *saw_wake = true;
            Ok(())
        }
        actual => mismatch(
            scenario,
            WakeCheck::Events,
            "configured resource or wake event",
            actual,
        ),
    }
}
