//! Poll ownership checks shared by query and mutation paths.

use crate::{
    Error, Registration, RegistrationInfo, RegistrationState, registration::PollId,
    table::RegistrationTable,
};

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
