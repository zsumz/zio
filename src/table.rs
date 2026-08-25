//! Fixed registration ownership and native-token translation.

use std::{
    num::NonZeroUsize,
    os::fd::{AsFd, AsRawFd, OwnedFd},
};

use crate::binding::{Binding, Observation};
use crate::token::{MAX_GENERATION, MAX_REGISTRATIONS, decode, encode};
use crate::{ArmState, Error, Interest, Key, Mode, RegistrationId, RegistrationState};

#[derive(Debug)]
struct Entry {
    descriptor: OwnedFd,
    key: Key,
    interest: Interest,
    mode: Mode,
    state: RegistrationState,
}

#[derive(Debug)]
struct Slot {
    generation: u32,
    entry: Option<Entry>,
    exhausted: bool,
}

impl Slot {
    const EMPTY: Self = Self {
        generation: 0,
        entry: None,
        exhausted: false,
    };
}

/// Owner-local fixed slot table.
#[derive(Debug)]
pub(crate) struct RegistrationTable {
    limit: NonZeroUsize,
    slots: Vec<Slot>,
    free: Vec<u32>,
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
        let mut free = Vec::new();
        free.try_reserve_exact(limit.get())
            .map_err(|_| Error::Capacity { limit: limit.get() })?;
        for index in 0..limit.get() {
            slots.push(Slot::EMPTY);
            free.push(u32::try_from(index).map_err(|_| Error::BackendOverflow)?);
        }
        free.reverse();
        Ok(Self {
            limit,
            slots,
            free,
            exhausted: 0,
        })
    }

    pub(crate) fn reserve(
        &mut self,
        descriptor: OwnedFd,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<RegistrationId, Error> {
        let Some(index) = self.free.pop() else {
            return if self.exhausted == self.limit.get() {
                Err(Error::RegistrationSpaceExhausted)
            } else {
                Err(Error::Capacity {
                    limit: self.limit.get(),
                })
            };
        };
        let slot = self
            .slots
            .get_mut(usize::try_from(index).map_err(|_| Error::Invariant)?)
            .ok_or(Error::Invariant)?;
        if slot.entry.is_some() || slot.exhausted {
            return Err(Error::Invariant);
        }
        slot.generation = slot
            .generation
            .checked_add(1)
            .ok_or(Error::RegistrationSpaceExhausted)?;
        let generation = core::num::NonZeroU32::new(slot.generation).ok_or(Error::Invariant)?;
        let id = encode(index, generation).ok_or(Error::RegistrationSpaceExhausted)?;
        slot.entry = Some(Entry {
            descriptor,
            key,
            interest,
            mode,
            state: RegistrationState::Registered {
                arm: ArmState::Armed,
            },
        });
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

    pub(crate) fn mark_disarmed(&mut self, id: RegistrationId) -> Result<(), Error> {
        let entry = self.entry_mut(id)?;
        if entry.mode == Mode::OneShot {
            entry.state = RegistrationState::Registered {
                arm: ArmState::Disarmed,
            };
        }
        Ok(())
    }

    pub(crate) fn mark_uncertain(&mut self, id: RegistrationId) -> Result<(), Error> {
        self.entry_mut(id)?.state = RegistrationState::Uncertain;
        Ok(())
    }

    pub(crate) fn retire(&mut self, id: RegistrationId) -> Result<(), Error> {
        let (index, generation) = decode(id)?;
        let slot = self
            .slots
            .get_mut(index)
            .ok_or(Error::Stale { registration: id })?;
        if slot.generation != generation.get() || slot.entry.take().is_none() {
            return Err(Error::Stale { registration: id });
        }
        if slot.generation == MAX_GENERATION {
            slot.exhausted = true;
            self.exhausted = self.exhausted.checked_add(1).ok_or(Error::Invariant)?;
        } else {
            self.free
                .push(u32::try_from(index).map_err(|_| Error::Invariant)?);
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
