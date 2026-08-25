//! Concrete scripted poller sharing the production mutation state machine.

use std::{
    num::NonZeroUsize,
    os::fd::{AsFd, AsRawFd},
};

use crate::{
    CommitStatus, DeleteError, Error, Interest, Key, Mode, RegisterError, Registration,
    RegistrationId, RegistrationState,
    mutation::{MutationSession, registration_state},
    poll::{DEFAULT_REGISTRATION_CAPACITY, next_poll_id},
    table::RegistrationTable,
};

use super::{
    MutationCall, MutationStep, ScriptError, ScriptedBackendState, driver::ScriptedDriver,
};

/// Concrete mutation-only poller driven by a finite backend script.
///
/// This type is compiled only with `test-support` and is intended for the
/// version-matched `zio-testkit`. It does not expose native descriptors or
/// implement waiting.
#[derive(Debug)]
pub struct ScriptedPoll {
    id: crate::registration::PollId,
    registrations: RegistrationTable,
    driver: ScriptedDriver,
}

impl ScriptedPoll {
    /// Creates a scripted poller with the default registration capacity.
    pub fn new(steps: impl IntoIterator<Item = MutationStep>) -> Result<Self, Error> {
        Self::with_capacity(DEFAULT_REGISTRATION_CAPACITY, steps)
    }

    /// Creates a scripted poller with a fixed registration capacity.
    pub fn with_capacity(
        registrations: usize,
        steps: impl IntoIterator<Item = MutationStep>,
    ) -> Result<Self, Error> {
        let capacity = NonZeroUsize::new(registrations).ok_or(Error::Capacity {
            limit: registrations,
        })?;
        Ok(Self {
            id: next_poll_id()?,
            registrations: RegistrationTable::new(capacity)?,
            driver: ScriptedDriver::new(steps),
        })
    }

    /// Registers one descriptor through the next scripted registration step.
    pub fn register<F: AsFd + ?Sized>(
        &mut self,
        source: &F,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        self.mutations().register(source, key, interest, mode)
    }

    /// Modifies one registration through the next scripted modification step.
    pub fn modify(
        &mut self,
        registration: &Registration,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), Error> {
        self.mutations().modify(registration, interest, mode)
    }

    /// Deletes one registration through the next scripted deletion step.
    pub fn delete(&mut self, registration: Registration) -> Result<(), DeleteError> {
        self.mutations().delete(registration)
    }

    /// Returns authoritative portable state for an owned registration.
    pub fn registration_state(
        &self,
        registration: &Registration,
    ) -> Result<RegistrationState, Error> {
        registration_state(self.id, &self.registrations, registration)
    }

    /// Establishes a delivered, disarmed one-shot state in both models.
    pub fn establish_disarmed(&mut self, registration: &Registration) -> Result<(), Error> {
        registration_state(self.id, &self.registrations, registration)?;
        let descriptor = {
            let binding = self.registrations.binding(registration.id(), false)?;
            if binding.mode != Mode::OneShot {
                return Err(Error::Invariant);
            }
            binding.descriptor.as_raw_fd()
        };
        self.driver
            .establish_disarmed(registration.id(), descriptor)
            .map_err(|_| Error::Invariant)?;
        self.registrations
            .apply_disarm(registration.id(), CommitStatus::Applied)
            .map(|_| ())
    }

    /// Borrows normalized calls observed by the scripted backend.
    pub fn calls(&self) -> &[MutationCall] {
        self.driver.calls()
    }

    /// Returns modeled backend state for an exact registration generation.
    pub fn backend_state(&self, registration: RegistrationId) -> ScriptedBackendState {
        self.driver.state(registration)
    }

    /// Proves that the script matched and every planned step was consumed.
    pub fn finish(&self) -> Result<(), ScriptError> {
        self.driver.finish()
    }

    fn mutations(&mut self) -> MutationSession<'_, ScriptedDriver> {
        MutationSession::new(self.id, &mut self.registrations, &mut self.driver)
    }
}
