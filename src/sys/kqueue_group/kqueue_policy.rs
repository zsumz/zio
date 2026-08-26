//! Atomicity policy for multi-filter kqueue mutations.

use std::{io, os::fd::RawFd};

use crate::{ArmState, Interest, Mode, RegistrationState, error::CommitStatus};

use super::super::failure::MutationFailure;
use super::{
    kqueue::Kqueue,
    kqueue_change::{Action, Change, ChangeList, Filter, Receipts},
};

pub(super) trait ChangeExecutor {
    fn apply(&self, changes: &ChangeList) -> io::Result<Receipts>;

    fn apply_registration(
        &self,
        descriptor: RawFd,
        token: u64,
        interest: Interest,
    ) -> io::Result<()> {
        let changes = additions(descriptor, token, interest, Action::AddEnabled);
        exact(self, &changes, false)
    }
}

impl ChangeExecutor for Kqueue {
    fn apply(&self, changes: &ChangeList) -> io::Result<Receipts> {
        Self::apply(self, changes)
    }

    fn apply_registration(
        &self,
        descriptor: RawFd,
        token: u64,
        interest: Interest,
    ) -> io::Result<()> {
        self.apply_registration_native(descriptor, token, interest)
    }
}

pub(super) fn register_descriptor<E: ChangeExecutor + ?Sized>(
    queue: &E,
    descriptor: RawFd,
    token: u64,
    interest: Interest,
) -> Result<(), MutationFailure> {
    match queue.apply_registration(descriptor, token, interest) {
        Ok(()) => Ok(()),
        Err(source_error) => {
            let commit = if cleanup(queue, descriptor).is_ok() {
                CommitStatus::NotApplied
            } else {
                CommitStatus::Unknown
            };
            Err(MutationFailure::new(commit, source_error))
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "rollback requires the complete prior state"
)]
pub(super) fn modify_descriptor<E: ChangeExecutor + ?Sized>(
    queue: &E,
    descriptor: RawFd,
    token: u64,
    previous_interest: Interest,
    _previous_mode: Mode,
    previous_arm: ArmState,
    desired_interest: Interest,
    _desired_mode: Mode,
) -> Result<(), MutationFailure> {
    let desired = replacement(descriptor, token, desired_interest, Action::AddEnabled);
    match exact(queue, &desired, true) {
        Ok(()) => Ok(()),
        Err(source_error) => {
            let restore_action = match previous_arm {
                ArmState::Armed => Action::AddEnabled,
                ArmState::Disarmed => Action::AddDisabled,
            };
            let prior = replacement(descriptor, token, previous_interest, restore_action);
            let commit = if exact(queue, &prior, true).is_ok() {
                CommitStatus::NotApplied
            } else {
                CommitStatus::Unknown
            };
            Err(MutationFailure::new(commit, source_error))
        }
    }
}

pub(super) fn delete_descriptor<E: ChangeExecutor + ?Sized>(
    queue: &E,
    descriptor: RawFd,
    interest: Interest,
    state: RegistrationState,
) -> Result<(), MutationFailure> {
    let exact_interest = match state {
        RegistrationState::Registered { .. } => interest,
        RegistrationState::Uncertain => Interest::READABLE.union(Interest::WRITABLE),
    };
    cleanup_interests(queue, descriptor, exact_interest)
        .map_err(|source| MutationFailure::new(CommitStatus::Unknown, source))
}

pub(super) fn cleanup<E: ChangeExecutor + ?Sized>(queue: &E, descriptor: RawFd) -> io::Result<()> {
    cleanup_interests(
        queue,
        descriptor,
        Interest::READABLE.union(Interest::WRITABLE),
    )
}

fn cleanup_interests<E: ChangeExecutor + ?Sized>(
    queue: &E,
    descriptor: RawFd,
    interest: Interest,
) -> io::Result<()> {
    let mut changes = ChangeList::new();
    push_interests(&mut changes, descriptor, 0, interest, Action::Delete);
    exact(queue, &changes, true)
}

pub(super) fn push_interests(
    changes: &mut ChangeList,
    descriptor: RawFd,
    token: u64,
    interest: Interest,
    action: Action,
) {
    if interest.is_readable() {
        let _ = changes.push(Change::new(descriptor, Filter::Read, action, token));
    }
    if interest.is_writable() {
        let _ = changes.push(Change::new(descriptor, Filter::Write, action, token));
    }
}

pub(super) fn exact<E: ChangeExecutor + ?Sized>(
    queue: &E,
    changes: &ChangeList,
    ignore_missing_delete: bool,
) -> io::Result<()> {
    let receipts = queue.apply(changes)?;
    receipts
        .iter()
        .find_map(|receipt| match receipt.error() {
            Some(libc::ENOENT) if ignore_missing_delete && receipt.action() == Action::Delete => {
                None
            }
            error => error,
        })
        .map_or(Ok(()), |code| Err(io::Error::from_raw_os_error(code)))
}

fn additions(descriptor: RawFd, token: u64, interest: Interest, action: Action) -> ChangeList {
    let mut changes = ChangeList::new();
    push_interests(&mut changes, descriptor, token, interest, action);
    changes
}

fn replacement(descriptor: RawFd, token: u64, interest: Interest, add: Action) -> ChangeList {
    let mut changes = ChangeList::new();
    let read = if interest.is_readable() {
        add
    } else {
        Action::Delete
    };
    let write = if interest.is_writable() {
        add
    } else {
        Action::Delete
    };
    let _ = changes.push(Change::new(descriptor, Filter::Read, read, token));
    let _ = changes.push(Change::new(descriptor, Filter::Write, write, token));
    changes
}
