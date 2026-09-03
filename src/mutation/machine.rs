//! Portable registration state transitions over a static mutation driver.

use std::os::fd::{AsFd, OwnedFd};

use crate::{
    ArmState, CommitStatus, DeleteError, Error, Interest, Key, Mode, MutationError, Operation,
    RegisterError, Registration, RegistrationState, descriptor::Descriptor,
    registration::PollOwner, table::RegistrationTable,
};

use super::{
    DeleteRequest, ModifyRequest, MutationDriver, authority::require_owner,
    register::register_descriptor as apply_registration,
};

/// Borrowed mutation state with a statically selected driver.
pub(crate) struct MutationSession<'state, Driver> {
    owner: &'state mut PollOwner,
    registrations: &'state mut RegistrationTable,
    driver: &'state mut Driver,
}

impl<'state, Driver: MutationDriver> MutationSession<'state, Driver> {
    pub(crate) const fn new(
        owner: &'state mut PollOwner,
        registrations: &'state mut RegistrationTable,
        driver: &'state mut Driver,
    ) -> Self {
        Self {
            owner,
            registrations,
            driver,
        }
    }

    pub(crate) fn register<F: AsFd + ?Sized>(
        &mut self,
        source: &F,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        validate_registration_interest(interest)?;
        let descriptor = source.as_fd().try_clone_to_owned().map_err(|source| {
            RegisterError::new(
                Error::Io {
                    operation: Operation::Register,
                    source,
                },
                None,
            )
        })?;
        self.register_descriptor(Descriptor::owned(descriptor), key, interest, mode)
    }

    pub(crate) fn register_owned(
        &mut self,
        source: OwnedFd,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        validate_registration_interest(interest)?;
        self.register_descriptor(Descriptor::owned(source), key, interest, mode)
    }

    /// Registers after erasing the source borrow into the retained table.
    ///
    /// # Safety
    ///
    /// The source must satisfy the complete lifetime and identity contract of
    /// [`crate::Poll::register_borrowed`].
    #[allow(
        unsafe_code,
        reason = "this internal seam propagates the public borrowed-descriptor contract"
    )]
    #[inline]
    pub(crate) unsafe fn register_borrowed<F: AsFd + ?Sized>(
        &mut self,
        source: &F,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        validate_registration_interest(interest)?;
        // SAFETY: this function requires the caller to uphold the descriptor
        // lifetime and identity invariant until the retained value is dropped.
        let descriptor = unsafe { Descriptor::borrowed(source.as_fd()) };
        self.register_descriptor(descriptor, key, interest, mode)
    }

    #[inline]
    fn register_descriptor(
        &mut self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        let Self {
            owner,
            registrations,
            driver,
        } = self;
        apply_registration(
            owner,
            registrations,
            &mut **driver,
            descriptor,
            key,
            interest,
            mode,
        )
    }

    pub(crate) fn modify(
        &mut self,
        registration: &Registration,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), Error> {
        require_owner(self.owner.current(), registration)?;
        if interest.is_empty() {
            return Err(Error::InvalidInterest);
        }
        let result = {
            let binding = self.registrations.binding(registration.id(), false)?;
            let RegistrationState::Registered { arm } = binding.state else {
                return Err(Error::Uncertain {
                    registration: registration.id(),
                });
            };
            self.driver.modify(ModifyRequest {
                descriptor: binding.descriptor,
                registration: registration.id(),
                previous_interest: binding.interest,
                previous_mode: binding.mode,
                previous_arm: arm,
                desired_interest: interest,
                desired_mode: mode,
            })
        };
        if let Err(failure) = result {
            match failure.commit() {
                CommitStatus::NotApplied => {}
                CommitStatus::Applied => {
                    self.registrations
                        .commit_modify(registration.id(), interest, mode)?;
                }
                CommitStatus::Unknown => {
                    self.registrations.mark_uncertain(registration.id())?;
                }
            }
            return Err(mutation_error(Operation::Modify, failure));
        }
        self.registrations
            .commit_modify(registration.id(), interest, mode)
    }

    pub(crate) fn rearm(&mut self, registration: &Registration) -> Result<(), Error> {
        require_owner(self.owner.current(), registration)?;
        let interest = {
            let binding = self.registrations.binding(registration.id(), false)?;
            match (binding.mode, binding.state) {
                (
                    Mode::OneShot,
                    RegistrationState::Registered {
                        arm: ArmState::Disarmed,
                    },
                ) => Some(binding.interest),
                (
                    _,
                    RegistrationState::Registered {
                        arm: ArmState::Armed,
                    },
                ) => None,
                _ => return Err(Error::Invariant),
            }
        };
        match interest {
            Some(interest) => self.modify(registration, interest, Mode::OneShot),
            None => Ok(()),
        }
    }

    #[inline]
    pub(crate) fn delete(&mut self, registration: Registration) -> Result<(), DeleteError> {
        if let Err(error) = require_owner(self.owner.current(), &registration) {
            return Err(DeleteError::new(error, registration));
        }
        let id = registration.id();
        let prepared = self
            .registrations
            .prepare_registration_retire(registration.encoded_id(), true)
            .map_err(|error| DeleteError::new(error, registration))?;
        let binding = prepared
            .binding()
            .map_err(|error| DeleteError::new(error, registration))?;
        let result = self.driver.delete(DeleteRequest {
            descriptor: binding.descriptor,
            registration: id,
            interest: binding.interest,
            state: binding.state,
        });
        if let Err(failure) = result {
            let state_result = match failure.commit() {
                CommitStatus::NotApplied => {
                    prepared.keep();
                    Ok(())
                }
                CommitStatus::Applied => prepared.retire(),
                CommitStatus::Unknown => prepared.mark_uncertain(),
            };
            if let Err(error) = state_result {
                return Err(DeleteError::new(error, registration));
            }
            return Err(DeleteError::new(
                mutation_error(Operation::Delete, failure),
                registration,
            ));
        }
        prepared
            .retire()
            .map_err(|error| DeleteError::new(error, registration))
    }
}

#[inline]
fn validate_registration_interest(interest: Interest) -> Result<(), RegisterError> {
    (!interest.is_empty())
        .then_some(())
        .ok_or(RegisterError::new(Error::InvalidInterest, None))
}

fn mutation_error(operation: Operation, failure: crate::sys::MutationFailure) -> Error {
    let commit = failure.commit();
    let source = failure.into_source();
    Error::Mutation(MutationError::new(operation, commit, source))
}
