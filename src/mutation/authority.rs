//! Poll ownership checks shared by query and mutation paths.

use std::os::fd::BorrowedFd;

use crate::{
    Error, Key, Registration, RegistrationInfo, RegistrationState, registration::PollId,
    table::RegistrationTable,
};

pub(crate) fn registration_fd<'poll>(
    owner: Option<PollId>,
    registrations: &'poll RegistrationTable,
    registration: &Registration,
) -> Result<BorrowedFd<'poll>, Error> {
    require_owner(owner, registration)?;
    registrations
        .binding(registration.id(), true)
        .map(|binding| binding.descriptor)
}

pub(crate) fn registrations(
    owner: Option<PollId>,
    registrations: &RegistrationTable,
) -> Result<Vec<Registration>, Error> {
    match owner {
        Some(owner) => registrations.snapshot(owner),
        None if registrations.len() == 0 => Ok(Vec::new()),
        None => Err(Error::Invariant),
    }
}

pub(crate) fn set_registration_key(
    owner: Option<PollId>,
    registrations: &mut RegistrationTable,
    registration: &Registration,
    key: Key,
) -> Result<(), Error> {
    require_owner(owner, registration)?;
    registrations.set_key(registration.id(), key)
}

pub(crate) fn registration_info(
    owner: Option<PollId>,
    registrations: &RegistrationTable,
    registration: &Registration,
) -> Result<RegistrationInfo, Error> {
    require_owner(owner, registration)?;
    registrations.info(registration.id())
}

pub(crate) fn registration_state(
    owner: Option<PollId>,
    registrations: &RegistrationTable,
    registration: &Registration,
) -> Result<RegistrationState, Error> {
    require_owner(owner, registration)?;
    registrations.state(registration.id())
}

pub(super) fn require_owner(
    owner: Option<PollId>,
    registration: &Registration,
) -> Result<(), Error> {
    if owner == Some(registration.owner()) {
        Ok(())
    } else {
        Err(Error::WrongPoller {
            registration: *registration,
        })
    }
}
