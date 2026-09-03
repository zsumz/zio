//! Scripted retained-registration membership.

use crate::Registration;

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
}
