//! Portable registration state transitions over a static mutation driver.

use std::os::fd::AsFd;

use crate::{
    CommitStatus, DeleteError, Error, Interest, Key, Mode, MutationError, Operation, RegisterError,
    Registration, RegistrationState,
    descriptor::Descriptor,
    registration::{PollId, PollOwner},
    table::RegistrationTable,
};

use super::{DeleteRequest, ModifyRequest, MutationDriver, RegisterRequest};

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

    fn register_descriptor(
        &mut self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        self.registrations
            .check_reservable()
            .map_err(|error| RegisterError::new(error, None))?;
        let owner = self
            .owner
            .get_or_assign()
            .map_err(|error| RegisterError::new(error, None))?;
        let id = self
            .registrations
            .reserve_descriptor(descriptor, key, interest, mode)
            .map_err(|error| RegisterError::new(error, None))?;
        let registration = Registration::new(owner, id);
        let result = {
            let binding = self
                .registrations
                .binding(id, false)
                .map_err(|error| RegisterError::new(error, None))?;
            self.driver.register(RegisterRequest {
                descriptor: binding.descriptor,
                registration: id,
                key,
                interest,
                mode,
            })
        };
        let Err(failure) = result else {
            return Ok(registration);
        };
        let commit = failure.commit();
        let error = mutation_error(Operation::Register, failure);
        match commit {
            CommitStatus::NotApplied => {
                self.registrations
                    .retire(id)
                    .map_err(|retire| RegisterError::new(retire, None))?;
                Err(RegisterError::new(error, None))
            }
            CommitStatus::Applied => Err(RegisterError::new(error, Some(registration))),
            CommitStatus::Unknown => {
                if let Err(state) = self.registrations.mark_uncertain(id) {
                    return Err(RegisterError::new(state, Some(registration)));
                }
                Err(RegisterError::new(error, Some(registration)))
            }
        }
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

    pub(crate) fn delete(&mut self, registration: Registration) -> Result<(), DeleteError> {
        if let Err(error) = require_owner(self.owner.current(), &registration) {
            return Err(DeleteError::new(error, registration));
        }
        let id = registration.id();
        let result = match self.registrations.binding(id, true) {
            Ok(binding) => self.driver.delete(DeleteRequest {
                descriptor: binding.descriptor,
                registration: id,
                interest: binding.interest,
                state: binding.state,
            }),
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
        self.registrations
            .retire(id)
            .map_err(|error| DeleteError::new(error, registration))
    }
}

fn validate_registration_interest(interest: Interest) -> Result<(), RegisterError> {
    (!interest.is_empty())
        .then_some(())
        .ok_or(RegisterError::new(Error::InvalidInterest, None))
}

pub(crate) fn registration_state(
    owner: Option<PollId>,
    registrations: &RegistrationTable,
    registration: &Registration,
) -> Result<RegistrationState, Error> {
    require_owner(owner, registration)?;
    registrations.state(registration.id())
}

fn require_owner(owner: Option<PollId>, registration: &Registration) -> Result<(), Error> {
    if owner == Some(registration.owner()) {
        Ok(())
    } else {
        Err(Error::WrongPoller {
            registration: registration.id(),
        })
    }
}

fn mutation_error(operation: Operation, failure: crate::sys::MutationFailure) -> Error {
    let commit = failure.commit();
    let source = failure.into_source();
    Error::Mutation(MutationError::new(operation, commit, source))
}
