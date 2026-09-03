//! Public registration operations over the static mutation state machine.

use std::{
    iter::FusedIterator,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
};

use crate::{
    DeleteAllError, DeleteError, DeleteOwnedError, Error, Interest, Key, Mode, Poll, RegisterError,
    RegisterOwnedError, Registration, RegistrationInfo, RegistrationState,
    mutation::{
        MutationSession, registration_fd, registration_info, registration_iter, registration_state,
        registrations, set_registration_key,
    },
};

impl Poll {
    /// Registers one descriptor with non-empty interest, retaining an owned duplicate.
    ///
    /// Every successful call creates an independent registration and exact
    /// generation, including repeated calls for the same source or duplicated
    /// handles for one open-file description.
    ///
    /// Success returns a copyable exact-generation handle. A reactor can retain
    /// one copy before giving another to cancellable work.
    ///
    /// On failure:
    ///
    /// - [`CommitStatus::NotApplied`](crate::CommitStatus::NotApplied) releases
    ///   the slot and duplicate, returning no registration;
    /// - [`CommitStatus::Applied`](crate::CommitStatus::Applied) returns an
    ///   armed registration; and
    /// - [`CommitStatus::Unknown`](crate::CommitStatus::Unknown) returns an
    ///   uncertain registration.
    pub fn register<F: AsFd + ?Sized>(
        &mut self,
        source: &F,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        self.mutations().register(source, key, interest, mode)
    }

    /// Registers one descriptor with non-empty interest by transferring ownership to the poller.
    ///
    /// This avoids descriptor duplication. A rejected or proven-not-applied
    /// call returns the original descriptor. Every other failure returns the
    /// handle under which the poller retained it.
    pub fn register_owned(
        &mut self,
        source: OwnedFd,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterOwnedError> {
        self.mutations().register_owned(source, key, interest, mode)
    }

    /// Replaces non-empty interest and mode, rearming a one-shot registration.
    ///
    /// Outcome semantics:
    ///
    /// - success or [`Applied`](crate::CommitStatus::Applied) commits the new
    ///   configuration and leaves it armed;
    /// - [`NotApplied`](crate::CommitStatus::NotApplied) preserves the complete
    ///   prior state; and
    /// - [`Unknown`](crate::CommitStatus::Unknown) makes the registration
    ///   uncertain, allowing only deletion.
    pub fn modify(
        &mut self,
        registration: &Registration,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), Error> {
        self.mutations().modify(registration, interest, mode)
    }

    /// Replaces key, non-empty interest, and mode, rearming a one-shot registration.
    ///
    /// Successful and `Applied` outcomes commit all three values. `NotApplied`
    /// preserves them; `Unknown` preserves the snapshot and marks it uncertain.
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

    /// Ensures a registration is armed without changing its interest or mode.
    ///
    /// A disarmed one-shot registration is modified and rearmed. Already-armed
    /// and level registrations return without backend work.
    pub fn rearm(&mut self, registration: &Registration) -> Result<(), Error> {
        self.mutations().rearm(registration)
    }

    /// Deletes a registration and releases its retained descriptor state.
    ///
    /// Success retires the exact generation, making every copy stale. Every
    /// failure returns the attempted handle through [`DeleteError`]:
    ///
    /// - [`NotApplied`](crate::CommitStatus::NotApplied) preserves its prior
    ///   state;
    /// - [`Applied`](crate::CommitStatus::Applied) retires it; and
    /// - [`Unknown`](crate::CommitStatus::Unknown) marks it uncertain and
    ///   permits another delete attempt.
    #[inline]
    pub fn delete(&mut self, registration: Registration) -> Result<(), DeleteError> {
        self.mutations().delete(registration)
    }

    /// Deletes a registration and returns its retained owned descriptor.
    ///
    /// Borrowed registrations are rejected without backend work. A
    /// proven-applied failure returns the descriptor; every other failure
    /// returns the exact attempted handle. Inspect the cause before reuse.
    pub fn delete_owned(
        &mut self,
        registration: Registration,
    ) -> Result<OwnedFd, DeleteOwnedError> {
        self.mutations().delete_owned(registration)
    }

    /// Deletes retained registrations until one deletion fails.
    ///
    /// The retained set is validated before deletion begins. On failure,
    /// earlier deletions may have succeeded and later entries are untouched.
    pub fn delete_all(&mut self) -> Result<(), DeleteAllError> {
        self.mutations().delete_all()
    }

    /// Returns whether this poller retains the exact registration.
    ///
    /// Uncertain registrations are retained. Stale and foreign handles are not.
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

    /// Returns authoritative state for a handle owned by this poller.
    pub fn registration_state(
        &self,
        registration: &Registration,
    ) -> Result<RegistrationState, Error> {
        registration_state(self.owner.current(), &self.registrations, registration)
    }

    /// Returns retained configuration and authoritative state.
    pub fn registration_info(
        &self,
        registration: &Registration,
    ) -> Result<RegistrationInfo, Error> {
        registration_info(self.owner.current(), &self.registrations, registration)
    }

    /// Borrows the descriptor retained for this registration.
    ///
    /// Uncertain backend state does not invalidate the descriptor.
    pub fn registration_fd<'poll>(
        &'poll self,
        registration: &Registration,
    ) -> Result<BorrowedFd<'poll>, Error> {
        registration_fd(self.owner.current(), &self.registrations, registration)
    }

    /// Returns an owned snapshot of every retained registration handle.
    ///
    /// The snapshot includes uncertain registrations and is bounded by
    /// [`Self::registration_capacity`]. Its order is unspecified.
    pub fn registrations(&self) -> Result<Vec<Registration>, Error> {
        registrations(self.owner.current(), &self.registrations)
    }

    /// Iterates retained registration handles without allocating.
    ///
    /// Uncertain registrations are included and order is unspecified.
    pub fn iter_registrations(
        &self,
    ) -> Result<
        impl DoubleEndedIterator<Item = Registration> + ExactSizeIterator + FusedIterator + '_,
        Error,
    > {
        registration_iter(self.owner.current(), &self.registrations)
    }

    /// Changes the key used by future events without backend work.
    ///
    /// Already-delivered events retain their key. Uncertain registrations may
    /// update this poller-local metadata.
    pub fn set_key(&mut self, registration: &Registration, key: Key) -> Result<(), Error> {
        set_registration_key(
            self.owner.current(),
            &mut self.registrations,
            registration,
            key,
        )
    }

    fn mutations(&mut self) -> MutationSession<'_, crate::sys::Backend> {
        MutationSession::new(&mut self.owner, &mut self.registrations, &mut self.backend)
    }
}

#[cfg(test)]
#[path = "registration_ops_test.rs"]
mod tests;
