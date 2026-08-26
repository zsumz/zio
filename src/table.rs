//! Fixed registration ownership and native-token translation.

use std::num::NonZeroUsize;

use crate::binding::{Binding, Observation};
use crate::descriptor::Descriptor;
use crate::token::{MAX_GENERATION, MAX_REGISTRATIONS, decode, encode};
use crate::{
    ArmState, CommitStatus, Error, Interest, Key, Mode, RegistrationId, RegistrationState,
};

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

    pub(crate) fn reserve_descriptor(
        &mut self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<RegistrationId, Error> {
        self.check_reservable()?;
        if self.free_head != FREE_END {
            return self.reserve_reused(descriptor, key, interest, mode);
        }
        let index = u32::try_from(self.slots.len()).map_err(|_| Error::BackendOverflow)?;
        let generation = core::num::NonZeroU32::new(1).ok_or(Error::Invariant)?;
        let id = encode(index, generation).ok_or(Error::RegistrationSpaceExhausted)?;
        let entry = Entry::registered(descriptor, key, interest, mode);
        self.slots.push(Slot::occupied(generation.get(), entry));
        Ok(id)
    }

    pub(crate) fn check_reservable(&self) -> Result<(), Error> {
        if self.free_head != FREE_END || self.slots.len() < self.limit.get() {
            return Ok(());
        }
        if self.exhausted == self.limit.get() {
            Err(Error::RegistrationSpaceExhausted)
        } else {
            Err(Error::Capacity {
                limit: self.limit.get(),
            })
        }
    }

    fn reserve_reused(
        &mut self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<RegistrationId, Error> {
        let index = self.free_head;
        let slot = self
            .slots
            .get_mut(usize::try_from(index).map_err(|_| Error::Invariant)?)
            .ok_or(Error::Invariant)?;
        if slot.entry.is_some() || slot.generation == MAX_GENERATION {
            return Err(Error::Invariant);
        }
        let next_generation = slot
            .generation
            .checked_add(1)
            .ok_or(Error::RegistrationSpaceExhausted)?;
        let generation = core::num::NonZeroU32::new(next_generation).ok_or(Error::Invariant)?;
        let id = encode(index, generation).ok_or(Error::RegistrationSpaceExhausted)?;
        self.free_head = slot.next_free;
        slot.next_free = FREE_END;
        slot.generation = next_generation;
        slot.entry = Some(Entry::registered(descriptor, key, interest, mode));
        Ok(id)
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

    pub(crate) fn retire(&mut self, id: RegistrationId) -> Result<(), Error> {
        let (index, generation) = decode(id)?;
        let free_index = u32::try_from(index).map_err(|_| Error::Invariant)?;
        let slot = self
            .slots
            .get_mut(index)
            .ok_or(Error::Stale { registration: id })?;
        if slot.generation != generation.get() || slot.entry.is_none() {
            return Err(Error::Stale { registration: id });
        }
        if slot.generation == MAX_GENERATION {
            let exhausted = self.exhausted.checked_add(1).ok_or(Error::Invariant)?;
            slot.entry = None;
            slot.next_free = FREE_END;
            self.exhausted = exhausted;
        } else {
            slot.entry = None;
            slot.next_free = self.free_head;
            self.free_head = free_index;
        }
        Ok(())
    }

    fn entry(&self, id: RegistrationId) -> Result<&Entry, Error> {
        let (index, generation) = decode(id)?;
        let slot = self
            .slots
            .get(index)
            .ok_or(Error::Stale { registration: id })?;
        if slot.generation != generation.get() {
            return Err(Error::Stale { registration: id });
        }
        slot.entry.as_ref().ok_or(Error::Stale { registration: id })
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
#[path = "table_test.rs"]
mod tests;
