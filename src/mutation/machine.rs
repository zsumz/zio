//! Portable mutation state transitions over a static driver.

use crate::{
    ArmState, CommitStatus, DeleteError, Error, Interest, Mode, MutationError, Operation,
    Registration, RegistrationState, registration::PollOwner, table::RegistrationTable,
};

use super::{DeleteRequest, ModifyRequest, MutationDriver, authority::require_owner};

/// Borrowed mutation state with a statically selected driver.
pub(crate) struct MutationSession<'state, Driver> {
    pub(super) owner: &'state mut PollOwner,
    pub(super) registrations: &'state mut RegistrationTable,
    pub(super) driver: &'state mut Driver,
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

fn mutation_error(operation: Operation, failure: crate::sys::MutationFailure) -> Error {
    let commit = failure.commit();
    let source = failure.into_source();
    Error::Mutation(MutationError::new(operation, commit, source))
}
