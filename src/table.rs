//! Fixed registration ownership and native-token translation.

use std::num::NonZeroUsize;

use crate::binding::{Binding, Observation};
use crate::token::{MAX_REGISTRATIONS, decode};
use crate::{ArmState, CommitStatus, Error, Interest, Mode, RegistrationId, RegistrationState};

#[path = "table_reserve.rs"]
mod reserve;
#[path = "table_retire.rs"]
mod retire;
#[path = "table_slot.rs"]
mod slot;

use slot::{Entry, FREE_END, Slot};

/// Owner-local fixed slot table.
#[derive(Debug)]
pub(crate) struct RegistrationTable {
    limit: NonZeroUsize,
    slots: Vec<Slot>,
    free_head: u32,
    exhausted: usize,
}

impl RegistrationTable {
    pub(crate) fn new(limit: NonZeroUsize) -> Result<Self, Error> {
        if limit.get() > MAX_REGISTRATIONS {
            return Err(Error::BackendOverflow);
        }
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(limit.get())
            .map_err(|_| Error::Capacity { limit: limit.get() })?;
        Ok(Self {
            limit,
            slots,
            free_head: FREE_END,
            exhausted: 0,
        })
    }

    pub(crate) fn binding(
        &self,
        id: RegistrationId,
        allow_uncertain: bool,
    ) -> Result<Binding<'_>, Error> {
        let entry = self.entry(id)?;
        if !allow_uncertain && entry.state == RegistrationState::Uncertain {
            return Err(Error::Uncertain { registration: id });
        }
        Ok(Binding {
            descriptor: entry.descriptor.as_fd(),
            interest: entry.interest,
            mode: entry.mode,
            state: entry.state,
        })
    }

    pub(crate) fn resolve(&self, token: u64) -> Option<Observation> {
        let id = RegistrationId::new(token);
        let entry = self.entry(id).ok()?;
        matches!(entry.state, RegistrationState::Registered { .. }).then_some(Observation {
            id,
            descriptor: entry.descriptor.as_raw_fd(),
            key: entry.key,
            #[cfg(target_os = "linux")]
            mode: entry.mode,
        })
    }

    pub(crate) fn state(&self, id: RegistrationId) -> Result<RegistrationState, Error> {
        self.entry(id).map(|entry| entry.state)
    }

    pub(crate) fn commit_modify(
        &mut self,
        id: RegistrationId,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), Error> {
        let entry = self.entry_mut(id)?;
        entry.interest = interest;
        entry.mode = mode;
        entry.state = RegistrationState::Registered {
            arm: ArmState::Armed,
        };
        Ok(())
    }

    pub(crate) fn mark_uncertain(&mut self, id: RegistrationId) -> Result<(), Error> {
        self.entry_mut(id)?.state = RegistrationState::Uncertain;
        Ok(())
    }

    pub(crate) fn apply_disarm(
        &mut self,
        id: RegistrationId,
        commit: CommitStatus,
    ) -> Result<RegistrationState, Error> {
        let entry = self.entry_mut(id)?;
        let armed = RegistrationState::Registered {
            arm: ArmState::Armed,
        };
        if entry.mode != Mode::OneShot || entry.state != armed {
            return Err(Error::Invariant);
        }
        entry.state = match commit {
            CommitStatus::Applied => RegistrationState::Registered {
                arm: ArmState::Disarmed,
            },
            CommitStatus::NotApplied => entry.state,
            CommitStatus::Unknown => RegistrationState::Uncertain,
        };
        Ok(entry.state)
    }

    fn entry(&self, id: RegistrationId) -> Result<&Entry, Error> {
        self.entry_with_index(id).map(|(_, entry)| entry)
    }

    #[inline]
    fn entry_with_index(&self, id: RegistrationId) -> Result<(usize, &Entry), Error> {
        let (index, generation) = decode(id)?;
        let slot = self
            .slots
            .get(index)
            .ok_or(Error::Stale { registration: id })?;
        if slot.generation != generation.get() {
            return Err(Error::Stale { registration: id });
        }
        let entry = slot
            .entry
            .as_ref()
            .ok_or(Error::Stale { registration: id })?;
        Ok((index, entry))
    }

    fn entry_mut(&mut self, id: RegistrationId) -> Result<&mut Entry, Error> {
        let (index, generation) = decode(id)?;
        let slot = self
            .slots
            .get_mut(index)
            .ok_or(Error::Stale { registration: id })?;
        if slot.generation != generation.get() {
            return Err(Error::Stale { registration: id });
        }
        slot.entry.as_mut().ok_or(Error::Stale { registration: id })
    }
}

#[cfg(test)]
#[path = "table_reserve_test.rs"]
mod reserve_tests;
#[cfg(test)]
#[path = "table_test.rs"]
mod tests;
