//! Reusable bounded event storage.

use std::{iter::FusedIterator, num::NonZeroUsize};

use crate::{CapacityKind, CapacityReason, Error, Event};

/// Fixed-capacity destination preserving native resource order.
/// A wake follows resource events, and registrations may share a key.
/// One destination may serve any poller whose event capacity it meets.
#[derive(Debug)]
pub struct Events {
    capacity: NonZeroUsize,
    events: Vec<Event>,
}

impl Events {
    /// Allocates an empty event destination with the supplied non-zero capacity.
    pub fn with_capacity(capacity: usize) -> Result<Self, Error> {
        let capacity = NonZeroUsize::new(capacity).ok_or(Error::Capacity {
            kind: CapacityKind::Event,
            limit: capacity,
            reason: CapacityReason::Zero,
        })?;
        Self::new(capacity)
    }

    pub(crate) fn new(capacity: NonZeroUsize) -> Result<Self, Error> {
        let mut events = Vec::new();
        events
            .try_reserve_exact(capacity.get())
            .map_err(|_| Error::Capacity {
                kind: CapacityKind::Event,
                limit: capacity.get(),
                reason: CapacityReason::StorageUnavailable,
            })?;
        Ok(Self { capacity, events })
    }

    /// Returns the fixed logical capacity.
    pub const fn capacity(&self) -> usize {
        self.capacity.get()
    }

    /// Returns the retained event count.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns whether no event is retained.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the number of unused logical event slots.
    pub fn remaining_capacity(&self) -> usize {
        self.capacity().saturating_sub(self.len())
    }

    /// Returns whether every logical event slot is occupied.
    pub fn is_full(&self) -> bool {
        self.remaining_capacity() == 0
    }

    /// Borrows retained events in normalized delivery order.
    pub fn as_slice(&self) -> &[Event] {
        &self.events
    }

    /// Borrows the event at `index`, when present.
    pub fn get(&self, index: usize) -> Option<&Event> {
        self.events.get(index)
    }

    /// Iterates from either end of the normalized delivery order.
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = &Event> + ExactSizeIterator + FusedIterator + '_ {
        self.events.iter()
    }

    /// Clears retained events while preserving allocated storage.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Drains retained events from either end of normalized delivery order.
    pub fn drain(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = Event> + ExactSizeIterator + FusedIterator + '_ {
        self.events.drain(..)
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn linux_storage(&mut self) -> &mut Vec<Event> {
        &mut self.events
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    pub(crate) fn try_push(&mut self, event: Event) -> Result<(), Error> {
        if self.events.len() >= self.capacity.get() {
            return Err(Error::EventsTooSmall {
                required: self.events.len().saturating_add(1),
                actual: self.capacity.get(),
            });
        }
        self.events.push(event);
        Ok(())
    }
}

impl<'a> IntoIterator for &'a Events {
    type Item = &'a Event;
    type IntoIter = core::slice::Iter<'a, Event>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.iter()
    }
}

impl IntoIterator for Events {
    type Item = Event;
    type IntoIter = std::vec::IntoIter<Event>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.into_iter()
    }
}

impl AsRef<[Event]> for Events {
    fn as_ref(&self) -> &[Event] {
        self.as_slice()
    }
}
