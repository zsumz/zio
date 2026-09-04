//! Scripted retained-registration inspection.

use std::iter::FusedIterator;

use crate::{Error, Registration, mutation::registration_iter};

use super::ScriptedPoll;

impl ScriptedPoll {
    /// Returns whether this poller retains the exact registration.
    pub fn contains(&self, registration: &Registration) -> bool {
        self.registration_state(registration).is_ok()
    }

    /// Returns whether this poller retains no registrations.
    pub const fn is_empty(&self) -> bool {
        self.registration_count() == 0
    }

    /// Returns whether no registration slot is currently reservable.
    pub const fn is_full(&self) -> bool {
        self.remaining_registration_capacity() == 0
    }

    /// Iterates retained registration handles without allocating.
    pub fn iter_registrations(
        &self,
    ) -> Result<
        impl DoubleEndedIterator<Item = Registration> + ExactSizeIterator + FusedIterator + '_,
        Error,
    > {
        registration_iter(self.owner.current(), &self.registrations)
    }
}
