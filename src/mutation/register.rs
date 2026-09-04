//! Fresh and reused registration transitions with isolated failure settlement.

use crate::{
    CommitStatus, Error, Interest, Key, Mode, MutationError, Operation, RegisterError,
    Registration,
    descriptor::Descriptor,
    registration::PollOwner,
    sys::MutationFailure,
    table::{RegistrationTable, Reservation},
};

use super::{MutationDriver, RegisterRequest};

#[derive(Debug)]
pub(crate) enum RegisterFailure {
    Released {
        error: Error,
        descriptor: Descriptor,
    },
    Retained {
        error: Error,
        registration: Registration,
    },
}

impl RegisterFailure {
    pub(crate) const fn released(error: Error, descriptor: Descriptor) -> Self {
        Self::Released { error, descriptor }
    }

    const fn retained(error: Error, registration: Registration) -> Self {
        Self::Retained {
            error,
            registration,
        }
    }

    pub(crate) fn discard_released(self) -> RegisterError {
        match self {
            Self::Released { error, descriptor } => {
                drop(descriptor);
                RegisterError::new(error, None)
            }
            Self::Retained {
                error,
                registration,
            } => RegisterError::new(error, Some(registration)),
        }
    }
}

#[inline]
pub(super) fn register_descriptor<Driver: MutationDriver>(
    owner: &mut PollOwner,
    registrations: &mut RegistrationTable,
    driver: &mut Driver,
    descriptor: Descriptor,
    key: Key,
    interest: Interest,
    mode: Mode,
) -> Result<Registration, RegisterFailure> {
    if registrations.has_virgin_slot() {
        return register_fresh(
            owner,
            registrations,
            driver,
            descriptor,
            key,
            interest,
            mode,
        );
    }
    if registrations.has_reusable_slot() {
        return register_reused(
            owner,
            registrations,
            driver,
            descriptor,
            key,
            interest,
            mode,
        );
    }
    register_fresh(
        owner,
        registrations,
        driver,
        descriptor,
        key,
        interest,
        mode,
    )
}

#[inline]
fn register_reused<Driver: MutationDriver>(
    owner: &mut PollOwner,
    registrations: &mut RegistrationTable,
    driver: &mut Driver,
    descriptor: Descriptor,
    key: Key,
    interest: Interest,
    mode: Mode,
) -> Result<Registration, RegisterFailure> {
    let permit = match registrations.reused_permit() {
        Ok(permit) => permit,
        Err(error) => return Err(RegisterFailure::released(error, descriptor)),
    };
    let owner = match owner.get_or_assign() {
        Ok(owner) => owner,
        Err(error) => return Err(RegisterFailure::released(error, descriptor)),
    };
    let registration = Registration::new(owner, permit.encoded_id());
    let reserved = permit.reserve_with(
        descriptor,
        key,
        interest,
        mode,
        |descriptor, registration| {
            driver.register(RegisterRequest {
                descriptor,
                registration,
                key,
                interest,
                mode,
            })
        },
    );
    let (reservation, native) = match reserved {
        Ok(reserved) => reserved,
        Err(failure) => {
            let (error, descriptor) = failure.into_parts();
            return Err(RegisterFailure::released(error, descriptor));
        }
    };
    match native {
        Ok(()) => Ok(reservation.keep(registration)),
        Err(failure) => settle_register_failure(reservation, registration, failure),
    }
}

fn register_fresh<Driver: MutationDriver>(
    owner: &mut PollOwner,
    registrations: &mut RegistrationTable,
    driver: &mut Driver,
    descriptor: Descriptor,
    key: Key,
    interest: Interest,
    mode: Mode,
) -> Result<Registration, RegisterFailure> {
    let permit = match registrations.fresh_permit() {
        Ok(permit) => permit,
        Err(error) => return Err(RegisterFailure::released(error, descriptor)),
    };
    let owner = match owner.get_or_assign() {
        Ok(owner) => owner,
        Err(error) => return Err(RegisterFailure::released(error, descriptor)),
    };
    let registration = Registration::new(owner, permit.encoded_id());
    let reserved = permit.reserve_with(
        descriptor,
        key,
        interest,
        mode,
        |descriptor, registration| {
            driver.register(RegisterRequest {
                descriptor,
                registration,
                key,
                interest,
                mode,
            })
        },
    );
    let (reservation, native) = match reserved {
        Ok(reserved) => reserved,
        Err(failure) => {
            let (error, descriptor) = failure.into_parts();
            return Err(RegisterFailure::released(error, descriptor));
        }
    };
    match native {
        Ok(()) => Ok(reservation.keep(registration)),
        Err(failure) => settle_register_failure(reservation, registration, failure),
    }
}

#[cold]
#[inline(never)]
fn settle_register_failure(
    reservation: Reservation<'_>,
    registration: Registration,
    failure: MutationFailure,
) -> Result<Registration, RegisterFailure> {
    let commit = failure.commit();
    let error = Error::Mutation(MutationError::new(
        Operation::Register,
        commit,
        failure.into_source(),
    ));
    match commit {
        CommitStatus::NotApplied => match reservation.release() {
            Ok(descriptor) => Err(RegisterFailure::released(error, descriptor)),
            Err(state) => Err(RegisterFailure::retained(state, registration)),
        },
        CommitStatus::Applied => Err(RegisterFailure::retained(error, registration)),
        CommitStatus::Unknown => {
            if let Err(state) = reservation.mark_uncertain() {
                return Err(RegisterFailure::retained(state, registration));
            }
            Err(RegisterFailure::retained(error, registration))
        }
    }
}

#[cfg(test)]
#[path = "register_test.rs"]
mod tests;
