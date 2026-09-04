//! Shared registration-table test operations.

use std::os::fd::OwnedFd;

use crate::{Error, Interest, Key, Mode, RegistrationId, descriptor::Descriptor};

use super::RegistrationTable;

impl RegistrationTable {
    pub(crate) fn reserve_descriptor(
        &mut self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<RegistrationId, Error> {
        let (id, reservation) = if self.has_virgin_slot() {
            let permit = self.fresh_permit()?;
            let id = permit.id();
            let (reservation, ()) = permit
                .reserve_with(descriptor, key, interest, mode, |_, _| ())
                .map_err(super::permit::ReservationFailure::discard_descriptor)?;
            (id, reservation)
        } else if self.has_reusable_slot() {
            let permit = self.reused_permit()?;
            let id = permit.id();
            let (reservation, ()) = permit
                .reserve_with(descriptor, key, interest, mode, |_, _| ())
                .map_err(super::permit::ReservationFailure::discard_descriptor)?;
            (id, reservation)
        } else {
            let permit = self.fresh_permit()?;
            let id = permit.id();
            let (reservation, ()) = permit
                .reserve_with(descriptor, key, interest, mode, |_, _| ())
                .map_err(super::permit::ReservationFailure::discard_descriptor)?;
            (id, reservation)
        };
        Ok(reservation.keep(id))
    }

    pub(crate) fn reserve(
        &mut self,
        descriptor: OwnedFd,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<RegistrationId, Error> {
        self.reserve_descriptor(Descriptor::owned(descriptor), key, interest, mode)
    }

    #[cfg_attr(
        not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )),
        allow(dead_code, reason = "matches supported retirement test support")
    )]
    pub(crate) fn retire(&mut self, id: RegistrationId) -> Result<(), Error> {
        self.prepare_retire(id, true)?.retire()
    }
}
