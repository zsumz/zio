//! Named poller construction.

use crate::Error;

use super::{DEFAULT_EVENT_CAPACITY, DEFAULT_REGISTRATION_CAPACITY, Poll};

/// Reusable named configuration for a [`Poll`].
#[must_use = "call build to create a poller"]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PollBuilder {
    event_capacity: usize,
    registration_capacity: usize,
}

impl PollBuilder {
    /// Creates a builder with the default capacities.
    pub const fn new() -> Self {
        Self {
            event_capacity: DEFAULT_EVENT_CAPACITY,
            registration_capacity: DEFAULT_REGISTRATION_CAPACITY,
        }
    }

    /// Sets the delivered-event capacity.
    pub const fn event_capacity(mut self, capacity: usize) -> Self {
        self.event_capacity = capacity;
        self
    }

    /// Sets the registration capacity.
    pub const fn registration_capacity(mut self, capacity: usize) -> Self {
        self.registration_capacity = capacity;
        self
    }

    /// Builds a poller from this configuration.
    pub fn build(self) -> Result<Poll, Error> {
        Poll::with_capacity(self.event_capacity, self.registration_capacity)
    }
}

impl Default for PollBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Poll {
    /// Returns a named poller builder with the default capacities.
    pub const fn builder() -> PollBuilder {
        PollBuilder::new()
    }
}

#[cfg(test)]
#[path = "poll_builder_test.rs"]
mod tests;
