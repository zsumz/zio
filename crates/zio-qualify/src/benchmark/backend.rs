//! Common candidate interface used by readiness workloads.

use std::{os::unix::net::UnixStream, time::Duration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Profile {
    InitialObservation,
    Level,
    OneShot,
}

pub(crate) trait Backend: Sized {
    type Registration<'source>
    where
        Self: 'source;

    fn new(event_capacity: usize, registration_capacity: usize) -> Result<Self, String>;

    fn construct_once(event_capacity: usize, registration_capacity: usize) -> Result<(), String> {
        drop(Self::new(event_capacity, registration_capacity)?);
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

    fn wake_roundtrip(&mut self, timeout: Duration) -> Result<u64, String>;
}

pub(crate) fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
