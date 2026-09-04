//! Descriptor-efficient kqueue fixture and measured observation cycles.

use std::{
    io::Write,
    os::{fd::OwnedFd, unix::net::UnixStream},
    time::Instant,
};

use zio::{Interest, Key, Mode, Poll, Wait, test_support::last_wait_metrics};

use super::{
    config::Row,
    model::{Measurement, RetainedMemory},
};

pub(super) fn measure(row: Row, mode: Mode) -> Result<Measurement, String> {
    let sources = Sources::new()?;
    let (mut setup, retained_memory) = measured_setup(&sources, row, mode)?;
    let mut seen = vec![false; row.ready];
    let mut distinct = 0_usize;
    let mut waits = 0_u64;
    let mut native_observations = 0_u64;
    let mut logical_events = 0_u64;
    let mut disarm_submissions = 0_u64;
    let mut disarmed_registrations = 0_u64;
    let mut disarm_elapsed_ns = 0_u128;
    let maximum_waits = row.ready.saturating_mul(2).saturating_add(1);
    let started = Instant::now();

    while distinct < row.ready {
        if usize::try_from(waits).map_or(true, |waits| waits >= maximum_waits) {
            return Err(format!(
                "ready cycle stalled after {waits} waits and {distinct} unique events"
            ));
        }
        setup
            .poll
            .wait(&mut setup.events, Wait::NoBlock)
            .map_err(display)?
            .into_result()
            .map_err(display)?;
        let metrics = last_wait_metrics(&setup.poll);
        waits = waits.saturating_add(1);
        native_observations = native_observations
            .saturating_add(u64::try_from(metrics.native_observations()).map_err(display)?);
        let disarms = u64::try_from(metrics.one_shot_disarms()).map_err(display)?;
        if disarms != 0 {
            disarm_submissions = disarm_submissions.saturating_add(1);
        }
        disarmed_registrations = disarmed_registrations.saturating_add(disarms);
        disarm_elapsed_ns = disarm_elapsed_ns.saturating_add(metrics.disarm_elapsed_ns());

        if setup.events.is_empty() {
            return Err(format!(
                "ready cycle returned no events after {distinct} unique observations"
            ));
        }
        for event in &setup.events {
            let key = usize::try_from(event.key()).map_err(display)?;
            let Some(was_seen) = seen.get_mut(key) else {
                return Err(format!("inactive or out-of-range readiness key {key}"));
            };
            if *was_seen && mode == Mode::OneShot {
                return Err(format!(
                    "one-shot readiness key {key} repeated after disarming"
                ));
            }
            if event.readiness().is_none() {
                return Err("unexpected wake in resource-only skew fixture".to_owned());
            }
            if !*was_seen {
                *was_seen = true;
                distinct = distinct.saturating_add(1);
            }
            logical_events = logical_events.saturating_add(1);
        }
    }

    Ok(Measurement {
        elapsed_ns: started.elapsed().as_nanos(),
        waits,
        native_observations,
        logical_events,
        unique_registrations: u64::try_from(distinct).map_err(display)?,
        disarm_submissions,
        disarmed_registrations,
        disarm_elapsed_ns,
        retained_memory,
    })
}

struct Setup {
    poll: Poll,
    events: zio::Events,
}

fn measured_setup(
    sources: &Sources,
    row: Row,
    mode: Mode,
) -> Result<(Setup, RetainedMemory), String> {
    let mut result = None;
    let allocations = allocation_counter::measure(|| {
        result = Some(build_setup(sources, row, mode));
    });
    let setup = result.ok_or_else(|| "skew setup did not execute".to_owned())??;
    Ok((
        setup,
        RetainedMemory {
            allocation_count: allocations.count_current,
            bytes: allocations.bytes_current,
            peak_bytes: allocations.bytes_max,
        },
    ))
}

fn build_setup(sources: &Sources, row: Row, mode: Mode) -> Result<Setup, String> {
    let mut poll = Poll::builder()
        .event_capacity(row.event_capacity)
        .registration_capacity(row.registrations)
        .build()
        .map_err(display)?;
    let events = poll.events().map_err(display)?;
    for index in 0..row.registrations {
        let source = if index < row.ready {
            &sources.ready
        } else {
            &sources.idle
        };
        let owned = OwnedFd::from(source.try_clone().map_err(display)?);
        let key = Key::try_from(index).map_err(display)?;
        let _registration = poll
            .register_owned(owned, key, Interest::READABLE, mode)
            .map_err(display)?;
    }
    Ok(Setup { poll, events })
}

struct Sources {
    ready: UnixStream,
    idle: UnixStream,
    _ready_peer: UnixStream,
    _idle_peer: UnixStream,
}

impl Sources {
    fn new() -> Result<Self, String> {
        let (ready, mut ready_peer) = UnixStream::pair().map_err(display)?;
        let (idle, idle_peer) = UnixStream::pair().map_err(display)?;
        ready_peer.write_all(&[1]).map_err(display)?;
        Ok(Self {
            ready,
            idle,
            _ready_peer: ready_peer,
            _idle_peer: idle_peer,
        })
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
