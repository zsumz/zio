//! Initially-ready single and batch workloads.

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use super::{
    backend::{Backend, Profile},
    measure::{Captured, Metric, capture},
    scenario::{Scenario, WAIT_TIMEOUT_MS},
};

pub(crate) fn ready_transaction<B: Backend>(
    scenario: Scenario,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let batch = scenario.batch_size();
    let (sources, peers) = pairs(batch)?;
    let mut backend = B::new(scenario.event_capacity(), scenario.registration_capacity())?;
    let mut registrations = Vec::with_capacity(batch);
    let mut seen = vec![false; batch];
    capture(iterations, metric, || {
        transaction(
            &mut backend,
            &sources,
            &peers,
            &mut registrations,
            &mut seen,
        )?;
        u64::try_from(batch).map_err(display)
    })
}

fn transaction<'source, B: Backend>(
    backend: &mut B,
    sources: &'source [UnixStream],
    peers: &[UnixStream],
    registrations: &mut Vec<B::Registration<'source>>,
    seen: &mut [bool],
) -> Result<(), String> {
    let result = register_all(backend, sources, registrations)
        .and_then(|()| signal_all(peers))
        .and_then(|()| collect_all(backend, sources, seen));
    let cleanup = delete_all(backend, registrations);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), _) | (Ok(()), Err(error)) => Err(error),
    }
}

fn register_all<'source, B: Backend>(
    backend: &mut B,
    sources: &'source [UnixStream],
    registrations: &mut Vec<B::Registration<'source>>,
) -> Result<(), String> {
    for (key, source) in sources.iter().enumerate() {
        registrations.push(backend.register(source, key, Profile::InitialObservation)?);
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

fn collect_all<B: Backend>(
    backend: &mut B,
    sources: &[UnixStream],
    seen: &mut [bool],
) -> Result<(), String> {
    seen.fill(false);
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(WAIT_TIMEOUT_MS))
        .ok_or_else(|| "ready deadline overflow".to_owned())?;
    let mut count = 0_usize;
    while count < sources.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!("ready batch timed out after {count} observations"));
        }
        backend.wait(remaining, &mut |key| {
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
            count = count.saturating_add(1);
            Ok(())
        })?;
    }
    Ok(())
}

fn delete_all<B: Backend>(
    backend: &mut B,
    registrations: &mut Vec<B::Registration<'_>>,
) -> Result<(), String> {
    let mut failure = None;
    while let Some(registration) = registrations.pop() {
        if let Err(error) = backend.delete(registration)
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
