//! Pretriggered and deliberately blocked cross-thread wake measurement.

use std::{
    sync::mpsc::{Receiver, SyncSender, sync_channel},
    thread,
    time::{Duration, Instant},
};

use super::{
    backend::{Backend, WakeHandle},
    measure::{Captured, FdProbe, LiveFds, Metric, capture, capture_latency},
    scenario::{BLOCKED_WAKE_SETTLE_US, WAIT_TIMEOUT_MS},
};

pub(crate) fn pretriggered<B: Backend>(
    event_capacity: usize,
    registration_capacity: usize,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let probe = FdProbe::discover();
    let fixture_baseline = probe.count();
    let mut backend = B::new(event_capacity, registration_capacity)?;
    let candidate_setup = probe.count();
    backend.configure_wake()?;
    let active = probe.count();
    let wake = backend.wake_handle()?;
    let captured = capture(iterations, metric, || {
        wake.wake()?;
        backend.wait_for_wake(Duration::from_millis(WAIT_TIMEOUT_MS))
    });
    drop(wake);
    drop(backend);
    let post_cleanup = probe.count();
    captured.map(|value| {
        value.with_live_fds(LiveFds::from_options(
            fixture_baseline,
            candidate_setup,
            active,
            post_cleanup,
        ))
    })
}

pub(crate) fn blocked<B: Backend>(
    event_capacity: usize,
    registration_capacity: usize,
    iterations: usize,
    metric: Option<Metric>,
) -> Result<Captured, String> {
    let probe = FdProbe::discover();
    let fixture_baseline = probe.count();
    let mut backend = B::new(event_capacity, registration_capacity)?;
    let candidate_setup = probe.count();
    backend.configure_wake()?;
    let active = probe.count();
    let wake = backend.wake_handle()?;
    let result = thread::scope(|scope| {
        let (arm_tx, arm_rx) = sync_channel(0);
        let (stamp_tx, stamp_rx) = sync_channel(0);
        let worker = scope.spawn(move || wake_worker(&wake, &arm_rx, &stamp_tx));
        let captured = capture_latency(iterations, metric, || {
            arm_tx.send(()).map_err(display)?;
            let observed = backend.wait_for_wake(Duration::from_millis(WAIT_TIMEOUT_MS));
            let returned = Instant::now();
            let stamp = stamp_rx.recv().map_err(display)??;
            let observed = observed?;
            Ok((
                observed,
                returned.saturating_duration_since(stamp).as_nanos(),
            ))
        });
        drop(arm_tx);
        let joined = worker
            .join()
            .map_err(|_| "blocked wake worker panicked".to_owned())?;
        combine(captured, joined)
    });
    drop(backend);
    let post_cleanup = probe.count();
    result.map(|value| {
        value.with_live_fds(LiveFds::from_options(
            fixture_baseline,
            candidate_setup,
            active,
            post_cleanup,
        ))
    })
}

fn wake_worker<W: WakeHandle>(
    wake: &W,
    arms: &Receiver<()>,
    stamps: &SyncSender<Result<Instant, String>>,
) -> Result<(), String> {
    while arms.recv().is_ok() {
        thread::sleep(Duration::from_micros(BLOCKED_WAKE_SETTLE_US));
        let started = Instant::now();
        let result = wake.wake().map(|()| started);
        stamps.send(result).map_err(display)?;
    }
    Ok(())
}

fn combine(
    captured: Result<Captured, String>,
    worker: Result<(), String>,
) -> Result<Captured, String> {
    match (captured, worker) {
        (Ok(captured), Ok(())) => Ok(captured),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
