//! Touched-slot coalescing for split kqueue filter observations.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use core::num::NonZeroUsize;

use crate::{Error, Key, Readiness, RegistrationId};

/// One logical resource observation in first-native-observation order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PendingResource {
    pub(crate) registration: RegistrationId,
    pub(crate) key: Key,
    pub(crate) readiness: Readiness,
}

#[derive(Debug)]
pub(crate) struct KqueuePending {
    capacity: NonZeroUsize,
    entries: Vec<PendingResource>,
    by_slot: Box<[Option<(RegistrationId, usize)>]>,
    touched: Vec<usize>,
}

impl KqueuePending {
    pub(crate) fn new(capacity: NonZeroUsize, registrations: NonZeroUsize) -> Result<Self, Error> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(capacity.get())
            .map_err(|_| Error::Capacity {
                limit: capacity.get(),
            })?;
        let mut by_slot = Vec::new();
        by_slot
            .try_reserve_exact(registrations.get())
            .map_err(|_| Error::Capacity {
                limit: registrations.get(),
            })?;
        by_slot.resize(registrations.get(), None);
        let touched_capacity = capacity.get().min(registrations.get());
        let mut touched = Vec::new();
        touched
            .try_reserve_exact(touched_capacity)
            .map_err(|_| Error::Capacity {
                limit: touched_capacity,
            })?;
        Ok(Self {
            capacity,
            entries,
            by_slot: by_slot.into_boxed_slice(),
            touched,
        })
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        for slot in self.touched.drain(..) {
            if let Some(entry) = self.by_slot.get_mut(slot) {
                *entry = None;
            }
        }
    }

    pub(crate) fn add(
        &mut self,
        registration: RegistrationId,
        key: Key,
        readiness: Readiness,
    ) -> Result<(), Error> {
        let slot = crate::token::decode(registration)?.0;
        let index = self.by_slot.get_mut(slot).ok_or(Error::Invariant)?;
        if let Some((observed, entry_index)) = *index {
            if observed != registration {
                return Err(Error::Invariant);
            }
            let entry = self.entries.get_mut(entry_index).ok_or(Error::Invariant)?;
            entry.readiness = entry.readiness.union(readiness);
            return Ok(());
        }
        if self.entries.len() >= self.capacity.get() {
            return Ok(());
        }
        let entry_index = self.entries.len();
        self.entries.push(PendingResource {
            registration,
            key,
            readiness,
        });
        *index = Some((registration, entry_index));
        self.touched.push(slot);
        Ok(())
    }

    pub(crate) fn as_slice(&self) -> &[PendingResource] {
        &self.entries
    }
}
