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

#[inline]
pub(super) fn register_descriptor<Driver: MutationDriver>(
    owner: &mut PollOwner,
    registrations: &mut RegistrationTable,
    driver: &mut Driver,
    descriptor: Descriptor,
    key: Key,
    interest: Interest,
    mode: Mode,
) -> Result<Registration, RegisterError> {
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
) -> Result<Registration, RegisterError> {
    let permit = match registrations.reused_permit() {
        Ok(permit) => permit,
        Err(error) => return register_state_failure(error),
    };
    let owner = match owner.get_or_assign() {
        Ok(owner) => owner,
        Err(error) => return register_state_failure(error),
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
        Err(error) => return register_state_failure(error),
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
) -> Result<Registration, RegisterError> {
    let permit = match registrations.fresh_permit() {
        Ok(permit) => permit,
        Err(error) => return register_state_failure(error),
    };
    let owner = match owner.get_or_assign() {
        Ok(owner) => owner,
        Err(error) => return register_state_failure(error),
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
        Err(error) => return register_state_failure(error),
    };
    match native {
        Ok(()) => Ok(reservation.keep(registration)),
        Err(failure) => settle_register_failure(reservation, registration, failure),
    }
}

#[cold]
#[inline(never)]
fn register_state_failure(error: Error) -> Result<Registration, RegisterError> {
    Err(RegisterError::new(error, None))
}

#[cold]
#[inline(never)]
fn settle_register_failure(
    reservation: Reservation<'_>,
    registration: Registration,
    failure: MutationFailure,
) -> Result<Registration, RegisterError> {
    let commit = failure.commit();
    let error = Error::Mutation(MutationError::new(
        Operation::Register,
        commit,
        failure.into_source(),
    ));
    match commit {
        CommitStatus::NotApplied => {
            if let Err(state) = reservation.retire() {
                return Err(RegisterError::new(state, None));
            }
            Err(RegisterError::new(error, None))
        }
        CommitStatus::Applied => Err(RegisterError::new(
            error,
            Some(reservation.keep(registration)),
        )),
        CommitStatus::Unknown => {
            if let Err(state) = reservation.mark_uncertain() {
                return Err(RegisterError::new(state, Some(registration)));
            }
            Err(RegisterError::new(error, Some(registration)))
        }
    }
}
