//! Wake coalescing, drain, and cloned-waker cases.

use std::{sync::mpsc, thread};

use core::time::Duration;

use zio::{Key, Wait};

use crate::wake_verify::{
    events, expect_empty, expect_single_wake, observed, poll, trigger, wait_for, waker,
};
use crate::{WakeCheck, WakeFailure, WakeScenario};

const STORM_KEY: Key = Key::new(4_101);
const ACROSS_WAIT_KEY: Key = Key::new(4_102);
const STORM_SIZE: usize = 256;
const OBSERVATION_LIMIT: Duration = Duration::from_secs(1);
const BLOCK_LIMIT: Duration = Duration::from_secs(2);

pub(crate) fn pre_wait_storm(scenario: WakeScenario) -> Result<(), WakeFailure> {
    let mut poll = poll(scenario, 1, 1)?;
    let waker = waker(&mut poll, STORM_KEY, scenario)?;
    let mut events = events(&poll, scenario)?;

    for index in 0..STORM_SIZE {
        waker.wake().map_err(|error| {
            observed(
                scenario,
                WakeCheck::Trigger,
                format!("successful wake {index} of {STORM_SIZE}"),
                &error,
            )
        })?;
    }
    wait_for(
        &mut poll,
        &mut events,
        Wait::For(OBSERVATION_LIMIT),
        scenario,
    )?;
    expect_single_wake(&events, STORM_KEY, scenario)?;
    wait_for(&mut poll, &mut events, Wait::NoBlock, scenario)?;
    expect_empty(&events, scenario)?;

    trigger(&waker, scenario)?;
    wait_for(
        &mut poll,
        &mut events,
        Wait::For(OBSERVATION_LIMIT),
        scenario,
    )?;
    expect_single_wake(&events, STORM_KEY, scenario)
}

pub(crate) fn clone_across_wait(scenario: WakeScenario) -> Result<(), WakeFailure> {
    let mut poll = poll(scenario, 1, 1)?;
    let waker = waker(&mut poll, ACROSS_WAIT_KEY, scenario)?;
    let wake_clone = waker.clone();
    let mut events = events(&poll, scenario)?;
    let (ready_sender, ready_receiver) = mpsc::sync_channel(0);
    let (go_sender, go_receiver) = mpsc::sync_channel(1);
    let (result_sender, result_receiver) = mpsc::sync_channel(0);
    let thread = thread::Builder::new()
        .name("zio-wake-conformance".to_owned())
        .spawn(move || {
            if ready_sender.send(()).is_err() {
                return;
            }
            if go_receiver.recv_timeout(OBSERVATION_LIMIT).is_err() {
                return;
            }
            let result = wake_clone.wake();
            let _result_ignored = result_sender.send(result);
        })
        .map_err(|error| observed(scenario, WakeCheck::Setup, "wake thread", &error))?;

    ready_receiver
        .recv_timeout(OBSERVATION_LIMIT)
        .map_err(|error| observed(scenario, WakeCheck::Deadline, "wake helper ready", &error))?;
    go_sender
        .send(())
        .map_err(|error| observed(scenario, WakeCheck::Trigger, "trigger request", &error))?;
    let wait_result = poll.wait(&mut events, Wait::For(BLOCK_LIMIT));
    let wake_result = result_receiver
        .recv_timeout(OBSERVATION_LIMIT)
        .map_err(|error| observed(scenario, WakeCheck::Deadline, "bounded wake result", &error))?;
    finish_thread(thread, scenario)?;
    wake_result
        .map_err(|error| observed(scenario, WakeCheck::Trigger, "successful wake", &error))?;
    wait_result
        .map_err(|error| observed(scenario, WakeCheck::Wait, "bounded wake wait", &error))?;
    expect_single_wake(&events, ACROSS_WAIT_KEY, scenario)
}

fn finish_thread(
    thread: thread::JoinHandle<()>,
    scenario: WakeScenario,
) -> Result<(), WakeFailure> {
    thread.join().map_err(|_| {
        WakeFailure::new(
            scenario,
            WakeCheck::Deadline,
            "clean wake helper exit",
            "wake helper thread panicked",
        )
    })
}
