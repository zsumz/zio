//! Registration acquisition, mutation, and release operations.

use std::os::fd::{AsFd, AsRawFd};

use crate::{
    CommitStatus, DeleteError, Error, Interest, Key, Mode, MutationError, Operation, Poll,
    RegisterError, Registration, RegistrationState,
};

impl Poll {
    /// Registers one descriptor after retaining an owned duplicate.
    pub fn register<F: AsFd + ?Sized>(
        &mut self,
        source: &F,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        if interest.is_empty() {
            return Err(RegisterError::new(Error::InvalidInterest, None));
        }
        let source_descriptor = source.as_fd().as_raw_fd();
        if let Some(existing) = self.registrations.duplicate(source_descriptor) {
            return Err(RegisterError::new(
                Error::Duplicate {
                    descriptor: source_descriptor,
                    existing,
                },
                None,
            ));
        }
        let descriptor = source.as_fd().try_clone_to_owned().map_err(|source| {
            RegisterError::new(
                Error::Io {
                    operation: Operation::Register,
                    source,
                },
                None,
            )
        })?;
        let id = self
            .registrations
            .reserve(source_descriptor, descriptor, key, interest, mode)
            .map_err(|error| RegisterError::new(error, None))?;
        let registration = Registration::new(self.id, id);
        let result = {
            let binding = self
                .registrations
                .binding(id, false)
                .map_err(|error| RegisterError::new(error, None))?;
            self.backend
                .register(binding.descriptor, id.get(), interest, mode)
        };
        if let Err(failure) = result {
            let commit = failure.commit();
            let error = mutation_error(Operation::Register, failure);
            if commit == CommitStatus::NotApplied {
                if let Err(retire_error) = self.registrations.retire(id) {
                    return Err(RegisterError::new(retire_error, None));
                }
                return Err(RegisterError::new(error, None));
            }
            if let Err(state_error) = self.registrations.mark_uncertain(id) {
                return Err(RegisterError::new(state_error, Some(registration)));
            }
            return Err(RegisterError::new(error, Some(registration)));
        }
        Ok(registration)
    }

    /// Replaces interest and mode, rearming a one-shot registration.
    pub fn modify(
        &mut self,
        registration: &Registration,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), Error> {
        self.require_owner(registration)?;
        if interest.is_empty() {
            return Err(Error::InvalidInterest);
        }
        let result = {
            let binding = self.registrations.binding(registration.id(), false)?;
            let arm = match binding.state {
                RegistrationState::Registered { arm } => arm,
                RegistrationState::Uncertain => {
                    return Err(Error::Uncertain {
                        registration: registration.id(),
                    });
                }
            };
            self.backend.modify(
                binding.descriptor,
                registration.id().get(),
                binding.interest,
                binding.mode,
                arm,
                interest,
                mode,
            )
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

    /// Deletes a registration and releases its retained descriptor.
    pub fn delete(&mut self, registration: Registration) -> Result<(), DeleteError> {
        if let Err(error) = self.require_owner(&registration) {
            return Err(DeleteError::new(error, registration));
        }
        let id = registration.id();
        let result = match self.registrations.binding(id, true) {
            Ok(binding) => self.backend.delete(binding.descriptor, binding.interest),
            Err(error) => return Err(DeleteError::new(error, registration)),
        };
        if let Err(failure) = result {
            let state_result = match failure.commit() {
                CommitStatus::NotApplied => Ok(()),
                CommitStatus::Applied => self.registrations.retire(id),
                CommitStatus::Unknown => self.registrations.mark_uncertain(id),
            };
            if let Err(error) = state_result {
                return Err(DeleteError::new(error, registration));
            }
            return Err(DeleteError::new(
                mutation_error(Operation::Delete, failure),
                registration,
            ));
        }
        match self.registrations.retire(id) {
            Ok(()) => Ok(()),
            Err(error) => Err(DeleteError::new(error, registration)),
        }
    }

    /// Returns authoritative state for a handle owned by this poller.
    pub fn registration_state(
        &self,
        registration: &Registration,
    ) -> Result<RegistrationState, Error> {
        self.require_owner(registration)?;
        self.registrations.state(registration.id())
    }

    fn require_owner(&self, registration: &Registration) -> Result<(), Error> {
        if registration.owner() == self.id {
            Ok(())
        } else {
            Err(Error::WrongPoller {
                registration: registration.id(),
            })
        }
    }
}

fn mutation_error(operation: Operation, failure: crate::sys::MutationFailure) -> Error {
    Error::Mutation(MutationError::new(
        operation,
        failure.commit(),
        failure.into_source(),
    ))
}
