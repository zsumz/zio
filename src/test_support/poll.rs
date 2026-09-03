//! Concrete scripted poller sharing the production mutation state machine.

use std::{
    num::NonZeroUsize,
    os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
};

use crate::{
    CapacityKind, CapacityReason, CommitStatus, DeleteError, Error, Interest, Key, Mode,
    RegisterError, RegisterOwnedError, Registration, RegistrationId, RegistrationInfo,
    RegistrationState,
    mutation::{
        MutationSession, registration_fd, registration_info, registration_state, registrations,
        set_registration_key,
    },
    poll::DEFAULT_REGISTRATION_CAPACITY,
    registration::PollOwner,
    table::RegistrationTable,
};

use super::{
    MutationCall, MutationStep, ScriptError, ScriptedBackendState, driver::ScriptedDriver,
};

/// Concrete mutation-only poller driven by a finite backend script.
///
/// This type is compiled only with `unstable-test-support` and is intended for the
/// version-matched `zio-testkit`. It does not expose native descriptors or
/// implement waiting.
#[derive(Debug)]
pub struct ScriptedPoll {
    owner: PollOwner,
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
            kind: CapacityKind::Registration,
            limit: registrations,
            reason: CapacityReason::Zero,
        })?;
        Ok(Self {
            owner: PollOwner::unassigned(),
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

    /// Registers one descriptor by transferring ownership through the next step.
    pub fn register_owned(
        &mut self,
        source: OwnedFd,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterOwnedError> {
        self.mutations().register_owned(source, key, interest, mode)
    }

    /// Registers one descriptor without duplicating it through the next step.
    ///
    /// # Safety
    ///
    /// The descriptor must remain open and retain its identity until deletion
    /// is proven applied or this scripted poller is dropped, matching
    /// [`crate::Poll::register_borrowed`].
    #[allow(
        unsafe_code,
        reason = "testkit callers explicitly assume the production borrowed descriptor contract"
    )]
    pub unsafe fn register_borrowed<F: AsFd + ?Sized>(
        &mut self,
        source: &F,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        // SAFETY: this unsafe test-support boundary carries the same obligation
        // as the production entrypoint into the shared mutation state machine.
        unsafe {
            self.mutations()
                .register_borrowed(source, key, interest, mode)
        }
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

    /// Rearms a disarmed one-shot registration through the next modify step.
    pub fn rearm(&mut self, registration: &Registration) -> Result<(), Error> {
        self.mutations().rearm(registration)
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
        registration_state(self.owner.current(), &self.registrations, registration)
    }

    /// Returns retained configuration and authoritative state.
    pub fn registration_info(
        &self,
        registration: &Registration,
    ) -> Result<RegistrationInfo, Error> {
        registration_info(self.owner.current(), &self.registrations, registration)
    }

    /// Borrows the descriptor retained for this registration.
    pub fn registration_fd<'poll>(
        &'poll self,
        registration: &Registration,
    ) -> Result<BorrowedFd<'poll>, Error> {
        registration_fd(self.owner.current(), &self.registrations, registration)
    }

    /// Returns an owned snapshot of every retained registration handle.
    pub fn registrations(&self) -> Result<Vec<Registration>, Error> {
        registrations(self.owner.current(), &self.registrations)
    }

    /// Changes the key retained for future modeled observations.
    pub fn set_key(&mut self, registration: &Registration, key: Key) -> Result<(), Error> {
        set_registration_key(
            self.owner.current(),
            &mut self.registrations,
            registration,
            key,
        )
    }

    /// Returns the fixed registration capacity.
    pub const fn registration_capacity(&self) -> usize {
        self.registrations.capacity()
    }

    /// Returns the retained registration count, including uncertain entries.
    pub const fn registration_count(&self) -> usize {
        self.registrations.len()
    }

    /// Returns the number of registration slots currently reservable.
    pub const fn remaining_registration_capacity(&self) -> usize {
        self.registrations.remaining()
    }

    /// Establishes a delivered, disarmed one-shot state in both models.
    pub fn establish_disarmed(&mut self, registration: &Registration) -> Result<(), Error> {
        registration_state(self.owner.current(), &self.registrations, registration)?;
        let descriptor = {
            let binding = self.registrations.binding(registration.id(), false)?;
            if !binding.mode.is_one_shot() {
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
        MutationSession::new(&mut self.owner, &mut self.registrations, &mut self.driver)
    }
}
