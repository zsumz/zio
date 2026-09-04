//! Scripted registration modification entrypoints.

use crate::{Error, Interest, Key, Mode, Registration};

use super::ScriptedPoll;

impl ScriptedPoll {
    /// Modifies one registration through the next scripted modification step.
    pub fn modify(
        &mut self,
        registration: &Registration,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), Error> {
        self.mutations().modify(registration, interest, mode)
    }

    /// Modifies key, interest, and mode through the next scripted step.
    pub fn modify_with_key(
        &mut self,
        registration: &Registration,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), Error> {
        self.mutations()
            .modify_with_key(registration, key, interest, mode)
    }

    /// Rearms a disarmed one-shot registration through the next modify step.
    pub fn rearm(&mut self, registration: &Registration) -> Result<(), Error> {
        self.mutations().rearm(registration)
    }
}
