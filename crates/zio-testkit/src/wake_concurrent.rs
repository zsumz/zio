//! Concurrent and repeated cross-thread wake conformance.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

use core::time::Duration;

use zio::{Key, Wait};

use crate::wake_verify::{
    events, expect_empty, expect_single_wake, observed, poll, trigger, wait_for, waker,
};
use crate::{WakeCheck, WakeFailure, WakeScenario};

const MULTI_PRODUCER_KEY: Key = Key::new(4_103);
const REPEATED_CROSS_THREAD_KEY: Key = Key::new(4_104);
const PRODUCER_COUNT: usize = 8;
const WAKES_PER_PRODUCER: usize = 512;
const CROSS_THREAD_ROUNDS: usize = 64;
const OBSERVATION_LIMIT: Duration = Duration::from_secs(1);
const BLOCK_LIMIT: Duration = Duration::from_secs(2);

pub(crate) fn multi_producer_storm(scenario: WakeScenario) -> Result<(), WakeFailure> {
    let mut poll = poll(scenario, 1, 1)?;
    let waker = waker(&mut poll, MULTI_PRODUCER_KEY, scenario)?;
    let mut events = events(&poll, scenario)?;
    let start = Arc::new(AtomicBool::new(false));
    let mut threads = Vec::new();
    threads.try_reserve_exact(PRODUCER_COUNT).map_err(|error| {
        observed(
            scenario,
            WakeCheck::Setup,
            "multi-producer thread storage",
            &error,
        )
    })?;

    for producer in 0..PRODUCER_COUNT {
        let wake_clone = waker.clone();
        let start_clone = Arc::clone(&start);
        let thread = thread::Builder::new()
            .name(format!("zio-wake-producer-{producer}"))
            .spawn(move || {
                while !start_clone.load(Ordering::Acquire) {
                    thread::yield_now();
                }
                for call in 0..WAKES_PER_PRODUCER {
                    wake_clone
                        .wake()
                        .map_err(|error| format!("producer {producer} wake {call}: {error}"))?;
                }
                Ok::<(), String>(())
            })
            .map_err(|error| {
                start.store(true, Ordering::Release);
                observed(scenario, WakeCheck::Setup, "multi-producer thread", &error)
            })?;
        threads.push(thread);
    }

    start.store(true, Ordering::Release);
    for thread in threads {
        finish_result_thread(thread, scenario, "successful concurrent wake producer")?;
    }
    wait_for(
        &mut poll,
        &mut events,
        Wait::For(OBSERVATION_LIMIT),
        scenario,
    )?;
    expect_single_wake(&events, MULTI_PRODUCER_KEY, scenario)?;
    wait_for(&mut poll, &mut events, Wait::NoBlock, scenario)?;
    expect_empty(&events, scenario)?;

    trigger(&waker, scenario)?;
    wait_for(
        &mut poll,
        &mut events,
        Wait::For(OBSERVATION_LIMIT),
        scenario,
    )?;
    expect_single_wake(&events, MULTI_PRODUCER_KEY, scenario)
}

pub(crate) fn repeated_cross_thread(scenario: WakeScenario) -> Result<(), WakeFailure> {
    let mut poll = poll(scenario, 1, 1)?;
    let waker = waker(&mut poll, REPEATED_CROSS_THREAD_KEY, scenario)?;
    let mut events = events(&poll, scenario)?;
    let (go_sender, go_receiver) = mpsc::sync_channel(0);
    let (result_sender, result_receiver) = mpsc::sync_channel(0);
    let thread = thread::Builder::new()
        .name("zio-wake-repeated".to_owned())
        .spawn(move || {
            for round in 0..CROSS_THREAD_ROUNDS {
                go_receiver
                    .recv()
                    .map_err(|error| format!("round {round} trigger request: {error}"))?;
                let result = waker
                    .wake()
                    .map_err(|error| format!("round {round} wake: {error}"));
                result_sender
                    .send(result)
                    .map_err(|error| format!("round {round} wake result: {error}"))?;
            }
            Ok::<(), String>(())
        })
        .map_err(|error| observed(scenario, WakeCheck::Setup, "repeated wake thread", &error))?;

    for round in 0..CROSS_THREAD_ROUNDS {
        go_sender.send(()).map_err(|error| {
            observed(
                scenario,
                WakeCheck::Trigger,
                format!("round {round} trigger request"),
                &error,
            )
        })?;
        wait_for(&mut poll, &mut events, Wait::For(BLOCK_LIMIT), scenario)?;
        let wake_result = result_receiver
            .recv_timeout(OBSERVATION_LIMIT)
            .map_err(|error| {
                observed(
                    scenario,
                    WakeCheck::Deadline,
                    format!("round {round} wake result"),
                    &error,
                )
            })?;
        wake_result.map_err(|error| {
            WakeFailure::new(
                scenario,
                WakeCheck::Trigger,
                format!("successful wake in round {round}"),
                error,
            )
        })?;
        expect_single_wake(&events, REPEATED_CROSS_THREAD_KEY, scenario)?;
        wait_for(&mut poll, &mut events, Wait::NoBlock, scenario)?;
        expect_empty(&events, scenario)?;
    }
    finish_result_thread(thread, scenario, "clean repeated wake helper exit")
}

fn finish_result_thread(
    thread: thread::JoinHandle<Result<(), String>>,
    scenario: WakeScenario,
    expected: &'static str,
) -> Result<(), WakeFailure> {
    match thread.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(WakeFailure::new(
            scenario,
            WakeCheck::Trigger,
            expected,
            error,
        )),
        Err(_) => Err(WakeFailure::new(
            scenario,
            WakeCheck::Deadline,
            expected,
            "wake helper thread panicked",
        )),
    }
}
