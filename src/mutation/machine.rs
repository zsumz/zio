//! Portable mutation state transitions over a static driver.

use crate::{
    ArmState, CommitStatus, Error, Interest, Key, Mode, MutationError, Operation, Registration,
    RegistrationState, registration::PollOwner, table::RegistrationTable,
};

use super::{ModifyRequest, MutationDriver, authority::require_owner};

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
        self.modify_configuration(registration, None, interest, mode)
    }

    pub(crate) fn modify_with_key(
        &mut self,
        registration: &Registration,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), Error> {
        self.modify_configuration(registration, Some(key), interest, mode)
    }

    fn modify_configuration(
        &mut self,
        registration: &Registration,
        key: Option<Key>,
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
        settle_modify(
            self.registrations,
            registration.id(),
            key,
            interest,
            mode,
            result,
        )
    }

    pub(crate) fn rearm(&mut self, registration: &Registration) -> Result<(), Error> {
        require_owner(self.owner.current(), registration)?;
        let modification = {
            let binding = self.registrations.binding(registration.id(), false)?;
            match (binding.mode, binding.state) {
                (
                    Mode::OneShot,
                    RegistrationState::Registered {
                        arm: ArmState::Disarmed,
                    },
                ) => Some((
                    binding.interest,
                    self.driver.modify(ModifyRequest {
                        descriptor: binding.descriptor,
                        registration: registration.id(),
                        previous_interest: binding.interest,
                        previous_mode: binding.mode,
                        previous_arm: ArmState::Disarmed,
                        desired_interest: binding.interest,
                        desired_mode: Mode::OneShot,
                    }),
                )),
                (
                    _,
                    RegistrationState::Registered {
                        arm: ArmState::Armed,
                    },
                ) => None,
                (_, RegistrationState::Uncertain) => {
                    return Err(Error::Uncertain {
                        registration: registration.id(),
                    });
                }
                (
                    Mode::Level,
                    RegistrationState::Registered {
                        arm: ArmState::Disarmed,
                    },
                ) => {
                    return Err(Error::Invariant);
                }
            }
        };
        match modification {
            Some((interest, result)) => settle_modify(
                self.registrations,
                registration.id(),
                None,
                interest,
                Mode::OneShot,
                result,
            ),
            None => Ok(()),
        }
    }
}

fn settle_modify(
    registrations: &mut RegistrationTable,
    registration: crate::RegistrationId,
    key: Option<Key>,
    interest: Interest,
    mode: Mode,
    result: Result<(), crate::sys::MutationFailure>,
) -> Result<(), Error> {
    if let Err(failure) = result {
        match failure.commit() {
            CommitStatus::NotApplied => {}
            CommitStatus::Applied => {
                registrations.commit_modify(registration, key, interest, mode)?;
            }
            CommitStatus::Unknown => {
                registrations.mark_uncertain(registration)?;
            }
        }
        return Err(mutation_error(Operation::Modify, failure));
    }
    registrations.commit_modify(registration, key, interest, mode)
}

pub(super) fn mutation_error(operation: Operation, failure: crate::sys::MutationFailure) -> Error {
    let commit = failure.commit();
    let source = failure.into_source();
    Error::Mutation(MutationError::new(operation, commit, source))
}
