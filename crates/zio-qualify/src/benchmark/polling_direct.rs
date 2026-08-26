//! Direct `polling` workloads that preserve its exact delivery semantics.

use std::{
    io::{Read, Write},
    num::NonZeroUsize,
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use polling::{Event, Events, PollMode, Poller};

use crate::polling_registration::PollingRegistration;

use super::{
    measure::{Captured, Metric, capture},
    scenario::{Scenario, WAIT_TIMEOUT_MS},
};

pub(crate) fn register_delete(
    _scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let (source, _peer) = UnixStream::pair().map_err(display)?;
    let poller = Poller::new().map_err(display)?;
    capture(iterations, metric, || {
        PollingRegistration::borrowed(&poller, &source, Event::readable(0), PollMode::Oneshot)
            .map_err(display)?
            .delete()
            .map_err(display)?;
        Ok(0)
    })
}

pub(crate) fn ready(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let batch = scenario.batch_size();
    let capacity = NonZeroUsize::new(scenario.event_capacity())
        .ok_or_else(|| "polling event capacity must be nonzero".to_owned())?;
    let (sources, peers) = pairs(batch)?;
    let poller = Poller::new().map_err(display)?;
    let mut events = Events::with_capacity(capacity);
    let mut registrations = Vec::with_capacity(batch);
    let mut seen = vec![false; batch];
    capture(iterations, metric, || {
        transaction(
            &poller,
            &mut events,
            &sources,
            &peers,
            &mut registrations,
            &mut seen,
        )?;
        u64::try_from(batch).map_err(display)
    })
}

fn transaction<'poller, 'source>(
    poller: &'poller Poller,
    events: &mut Events,
    sources: &'source [UnixStream],
    peers: &[UnixStream],
    registrations: &mut Vec<PollingRegistration<'poller, 'source>>,
    seen: &mut [bool],
) -> Result<(), String> {
    let result = register_all(poller, sources, registrations)
        .and_then(|()| signal_all(peers))
        .and_then(|()| collect_all(poller, events, sources, seen));
    let cleanup = delete_all(registrations);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), _) | (Ok(()), Err(error)) => Err(error),
    }
}

fn register_all<'poller, 'source>(
    poller: &'poller Poller,
    sources: &'source [UnixStream],
    registrations: &mut Vec<PollingRegistration<'poller, 'source>>,
) -> Result<(), String> {
    for (key, source) in sources.iter().enumerate() {
        registrations.push(
            PollingRegistration::borrowed(poller, source, Event::readable(key), PollMode::Oneshot)
                .map_err(display)?,
        );
    }
    Ok(())
}

fn signal_all(peers: &[UnixStream]) -> Result<(), String> {
    for peer in peers {
        let mut peer = peer;
        peer.write_all(&[1]).map_err(display)?;
    }
    Ok(())
}

fn collect_all(
    poller: &Poller,
    events: &mut Events,
    sources: &[UnixStream],
    seen: &mut [bool],
) -> Result<(), String> {
    seen.fill(false);
    let mut observe = |key| observe_key(key, sources, seen);
    collect_events(poller, events, sources.len(), &mut observe)
}

fn collect_events(
    poller: &Poller,
    events: &mut Events,
    expected: usize,
    observe: &mut dyn FnMut(usize) -> Result<(), String>,
) -> Result<(), String> {
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(WAIT_TIMEOUT_MS))
        .ok_or_else(|| "ready deadline overflow".to_owned())?;
    let mut count = 0_usize;
    while count < expected {
        events.clear();
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!("ready batch timed out after {count} observations"));
        }
        poller.wait(events, Some(remaining)).map_err(display)?;
        for event in events.iter() {
            if !event.readable {
                return Err(format!("polling key {} was not readable", event.key));
            }
            observe(event.key)?;
            count = count.saturating_add(1);
        }
    }
    Ok(())
}

fn observe_key(key: usize, sources: &[UnixStream], seen: &mut [bool]) -> Result<(), String> {
    let was_seen = seen
        .get_mut(key)
        .ok_or_else(|| format!("out-of-range readiness key {key}"))?;
    if *was_seen {
        return Err(format!("duplicate readiness key {key}"));
    }
    let source = sources
        .get(key)
        .ok_or_else(|| format!("missing source for key {key}"))?;
    let mut source = source;
    source.read_exact(&mut [0]).map_err(display)?;
    *was_seen = true;
    Ok(())
}

fn delete_all(registrations: &mut Vec<PollingRegistration<'_, '_>>) -> Result<(), String> {
    let mut failure = None;
    while let Some(registration) = registrations.pop() {
        if let Err(error) = registration.delete().map_err(display)
            && failure.is_none()
        {
            failure = Some(error);
        }
    }
    failure.map_or(Ok(()), Err)
}

fn pairs(count: usize) -> Result<(Vec<UnixStream>, Vec<UnixStream>), String> {
    let mut sources = Vec::with_capacity(count);
    let mut peers = Vec::with_capacity(count);
    for _ in 0..count {
        let (source, peer) = UnixStream::pair().map_err(display)?;
        sources.push(source);
        peers.push(peer);
    }
    Ok((sources, peers))
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
