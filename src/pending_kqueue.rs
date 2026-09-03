//! Sparse-slot coalescing in first-observation order for kqueue filters.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use core::{num::NonZeroUsize, ops::Range};

use crate::{CapacityKind, CapacityReason, Error, Key, Readiness, RegistrationId};

const EMPTY: u32 = u32::MAX;

/// One logical resource observation in first-native-observation order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PendingResource {
    pub(crate) registration: RegistrationId,
    pub(crate) key: Key,
    pub(crate) readiness: Readiness,
}

#[derive(Debug)]
pub(crate) struct KqueuePending {
    entries: Vec<PendingResource>,
    by_slot: Box<[u32]>,
    last_delivered: Option<RegistrationId>,
}

impl KqueuePending {
    pub(crate) fn new(registrations: NonZeroUsize) -> Result<Self, Error> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(registrations.get())
            .map_err(|_| Error::Capacity {
                kind: CapacityKind::Registration,
                limit: registrations.get(),
                reason: CapacityReason::StorageUnavailable,
            })?;
        let mut by_slot = Vec::new();
        by_slot
            .try_reserve_exact(registrations.get())
            .map_err(|_| Error::Capacity {
                kind: CapacityKind::Registration,
                limit: registrations.get(),
                reason: CapacityReason::StorageUnavailable,
            })?;
        by_slot.resize(registrations.get(), EMPTY);
        Ok(Self {
            entries,
            by_slot: by_slot.into_boxed_slice(),
            last_delivered: None,
        })
    }

    pub(crate) fn clear(&mut self) {
        let mut valid = true;
        for (entry_index, pending) in self.entries.iter().enumerate() {
            let Ok((slot, _generation)) = crate::token::decode(pending.registration) else {
                valid = false;
                break;
            };
            let Some(index) = self.by_slot.get_mut(slot) else {
                valid = false;
                break;
            };
            if usize::try_from(*index).ok() != Some(entry_index) {
                valid = false;
                break;
            }
            *index = EMPTY;
        }
        if !valid {
            self.by_slot.fill(EMPTY);
        }
        self.entries.clear();
    }

    pub(crate) fn add(
        &mut self,
        registration: RegistrationId,
        key: Key,
        readiness: Readiness,
    ) -> Result<(), Error> {
        let slot = crate::token::decode(registration)?.0;
        let slot_count = self.by_slot.len();
        let index = self.by_slot.get_mut(slot).ok_or(Error::Invariant)?;
        if *index != EMPTY {
            let entry_index = usize::try_from(*index).map_err(|_| Error::Invariant)?;
            let entry = self.entries.get_mut(entry_index).ok_or(Error::Invariant)?;
            if entry.registration != registration {
                return Err(Error::Invariant);
            }
            entry.readiness = entry.readiness.union(readiness);
            return Ok(());
        }
        if self.entries.len() >= slot_count {
            return Err(Error::Invariant);
        }
        let entry_index = self.entries.len();
        let retained_index = u32::try_from(entry_index).map_err(|_| Error::Invariant)?;
        self.entries.push(PendingResource {
            registration,
            key,
            readiness,
        });
        *index = retained_index;
        Ok(())
    }

    /// Selects the next non-wrapping window in observation order.
    pub(crate) fn delivery_range(&mut self, limit: usize) -> Range<usize> {
        let len = self.entries.len();
        let start = if limit >= len {
            0
        } else {
            self.last_delivered
                .and_then(|registration| {
                    let slot = crate::token::decode(registration).ok()?.0;
                    let entry_index = usize::try_from(*self.by_slot.get(slot)?).ok()?;
                    let entry = self.entries.get(entry_index)?;
                    (entry.registration == registration).then_some(if entry_index + 1 == len {
                        0
                    } else {
                        entry_index + 1
                    })
                })
                .unwrap_or(0)
        };
        let end = start.saturating_add(limit).min(len);
        if start < end {
            self.last_delivered = Some(self.entries[end - 1].registration);
        }
        start..end
    }

    pub(crate) fn as_slice(&self) -> &[PendingResource] {
        &self.entries
    }
}
