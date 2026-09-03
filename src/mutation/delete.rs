//! Descriptor-preserving deletion settlement.

use crate::{CommitStatus, DeleteError, Error, Operation, Registration, descriptor::Descriptor};

use super::{
    DeleteRequest, MutationDriver, MutationSession, authority::require_owner,
    machine::mutation_error,
};

#[derive(Debug)]
pub(crate) enum DeleteFailure {
    Released {
        error: Error,
        registration: Registration,
        descriptor: Descriptor,
    },
    Retained {
        error: Error,
        registration: Registration,
    },
}

impl DeleteFailure {
    fn released(error: Error, registration: Registration, descriptor: Descriptor) -> Self {
        Self::Released {
            error,
            registration,
            descriptor,
        }
    }

    fn retained(error: Error, registration: Registration) -> Self {
        Self::Retained {
            error,
            registration,
        }
    }

    pub(crate) fn discard_released(self) -> DeleteError {
        let (error, registration) = match self {
            Self::Released {
                error,
                registration,
                descriptor,
            } => {
                drop(descriptor);
                (error, registration)
            }
            Self::Retained {
                error,
                registration,
            } => (error, registration),
        };
        DeleteError::new(error, registration)
    }
}

impl<Driver: MutationDriver> MutationSession<'_, Driver> {
    #[inline]
    pub(crate) fn delete(&mut self, registration: Registration) -> Result<(), DeleteError> {
        self.delete_descriptor(registration)
            .map(drop)
            .map_err(DeleteFailure::discard_released)
    }

    pub(crate) fn delete_descriptor(
        &mut self,
        registration: Registration,
    ) -> Result<Descriptor, DeleteFailure> {
        if let Err(error) = require_owner(self.owner.current(), &registration) {
            return Err(DeleteFailure::retained(error, registration));
        }
        let id = registration.id();
        let prepared = self
            .registrations
            .prepare_registration_retire(registration.encoded_id(), true)
            .map_err(|error| DeleteFailure::retained(error, registration))?;
        let binding = prepared
            .binding()
            .map_err(|error| DeleteFailure::retained(error, registration))?;
        let result = self.driver.delete(DeleteRequest {
            descriptor: binding.descriptor,
            registration: id,
            interest: binding.interest,
            state: binding.state,
        });
        let Err(failure) = result else {
            return prepared
                .release()
                .map_err(|error| DeleteFailure::retained(error, registration));
        };
        let commit = failure.commit();
        let error = mutation_error(Operation::Delete, failure);
        match commit {
            CommitStatus::NotApplied => {
                prepared.keep();
                Err(DeleteFailure::retained(error, registration))
            }
            CommitStatus::Applied => match prepared.release() {
                Ok(descriptor) => Err(DeleteFailure::released(error, registration, descriptor)),
                Err(error) => Err(DeleteFailure::retained(error, registration)),
            },
            CommitStatus::Unknown => {
                if let Err(error) = prepared.mark_uncertain() {
                    return Err(DeleteFailure::retained(error, registration));
                }
                Err(DeleteFailure::retained(error, registration))
            }
        }
    }
}

#[cfg(test)]
#[path = "delete_test.rs"]
mod tests;
