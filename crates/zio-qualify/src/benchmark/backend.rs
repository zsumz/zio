//! Common candidate interface used by readiness workloads.

use std::{os::unix::net::UnixStream, time::Duration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Profile {
    InitialObservation,
    Persistent,
    Level,
    OneShot,
}

pub(crate) trait Backend: Sized {
    type Registration<'source>
    where
        Self: 'source;
    type Wake: WakeHandle;

    fn new(event_capacity: usize, registration_capacity: usize) -> Result<Self, String>;

    fn construct_once(event_capacity: usize, registration_capacity: usize) -> Result<(), String> {
        drop(Self::new(event_capacity, registration_capacity)?);
        Ok(())
    }

    fn construct_with_waker_once(
        event_capacity: usize,
        registration_capacity: usize,
    ) -> Result<(), String> {
        let mut backend = Self::new(event_capacity, registration_capacity)?;
        backend.configure_wake()?;
        drop(backend.wake_handle()?);
        Ok(())
    }

    fn register<'source>(
        &mut self,
        source: &'source UnixStream,
        key: usize,
        profile: Profile,
    ) -> Result<Self::Registration<'source>, String>;

    fn rearm(
        &mut self,
        registration: &Self::Registration<'_>,
        profile: Profile,
    ) -> Result<(), String>;

    fn delete(&mut self, registration: Self::Registration<'_>) -> Result<(), String>;

    fn wait(
        &mut self,
        timeout: Duration,
        observe: &mut dyn FnMut(usize) -> Result<(), String>,
    ) -> Result<usize, String>;

    fn configure_wake(&mut self) -> Result<(), String>;

    fn wake_handle(&self) -> Result<Self::Wake, String>;

    fn wait_for_wake(&mut self, timeout: Duration) -> Result<u64, String>;
}

pub(crate) trait WakeHandle: Send + 'static {
    fn wake(&self) -> Result<(), String>;
}

pub(crate) fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
