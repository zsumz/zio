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
    /// Registers one descriptor after retaining an owned duplicate.
    ///
    /// Every successful call creates an independent registration and exact
    /// generation, including repeated calls for the same source or duplicated
    /// handles for one open-file description.
    ///
    /// A successful call returns a copyable, exact-generation handle for the
    /// new registration. A reactor can retain one copy before giving another
    /// to cancellable work. If the backend reports
    /// [`CommitStatus::NotApplied`], the reserved slot and retained descriptor
    /// are released and [`RegisterError::registration`] returns `None`. An
    /// [`CommitStatus::Applied`] failure returns a registered, armed handle; an
    /// [`CommitStatus::Unknown`] failure returns a handle whose authoritative
    /// state is [`RegistrationState::Uncertain`].
    ///
    /// [`CommitStatus::Applied`]: crate::CommitStatus::Applied
    /// [`CommitStatus::NotApplied`]: crate::CommitStatus::NotApplied
    /// [`CommitStatus::Unknown`]: crate::CommitStatus::Unknown
    pub fn register<F: AsFd + ?Sized>(
        &mut self,
        source: &F,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        self.mutations().register(source, key, interest, mode)
    }

    /// Registers one descriptor by transferring ownership to the poller.
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

    /// Replaces interest and mode, rearming a one-shot registration.
    ///
    /// A successful call, or a failure classified as
    /// [`CommitStatus::Applied`], commits the desired interest and mode and
    /// leaves the registration armed. [`CommitStatus::NotApplied`] preserves
    /// the complete prior interest, mode, and arm state. An unknown outcome
    /// makes the registration uncertain and prevents later modification until
    /// it is explicitly deleted.
    ///
    /// [`CommitStatus::Applied`]: crate::CommitStatus::Applied
    /// [`CommitStatus::NotApplied`]: crate::CommitStatus::NotApplied
    pub fn modify(
        &mut self,
        registration: &Registration,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), Error> {
        self.mutations().modify(registration, interest, mode)
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
    /// Success retires the exact generation and makes every remaining handle
    /// copy stale. Every failed deletion retains the exact handle through
    /// [`DeleteError`]. A
    /// [`CommitStatus::NotApplied`] failure preserves the prior authoritative
    /// state for retry; an [`CommitStatus::Applied`] failure retires the state,
    /// so every copy is stale; an [`CommitStatus::Unknown`] failure marks the
    /// registration uncertain and permits an explicit delete retry from any
    /// copy.
    ///
    /// [`CommitStatus::Applied`]: crate::CommitStatus::Applied
    /// [`CommitStatus::NotApplied`]: crate::CommitStatus::NotApplied
    /// [`CommitStatus::Unknown`]: crate::CommitStatus::Unknown
    #[inline]
    pub fn delete(&mut self, registration: Registration) -> Result<(), DeleteError> {
        self.mutations().delete(registration)
    }

    /// Deletes a registration and returns its retained owned descriptor.
    ///
    /// Borrowed registrations are rejected without backend work. A
    /// proven-applied failure returns the descriptor; every other failure
    /// retains the exact registration handle.
    pub fn delete_owned(
        &mut self,
        registration: Registration,
    ) -> Result<OwnedFd, DeleteOwnedError> {
        self.mutations().delete_owned(registration)
    }

    /// Deletes retained registrations until one deletion fails.
    ///
    /// A bounded snapshot is taken before deletion begins. On failure, earlier
    /// deletions may have succeeded and later snapshot entries are untouched.
    pub fn delete_all(&mut self) -> Result<(), DeleteAllError> {
        let registrations = self.registrations().map_err(DeleteAllError::snapshot)?;
        for registration in registrations {
            self.delete(registration)
                .map_err(DeleteAllError::deletion)?;
        }
        Ok(())
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
