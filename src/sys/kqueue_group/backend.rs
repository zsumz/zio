//! Safe receipt-checked kqueue backend and event normalization.

use std::{
    io,
    os::fd::{AsRawFd, BorrowedFd, RawFd},
    sync::Arc,
};

use crate::{
    ArmState, Interest, Mode, Readiness, Wait,
    error::{CommitStatus, Operation},
};

use super::super::{
    event::RawEvent,
    failure::{MutationFailure, SetupFailure},
};
use super::{
    kqueue::{KeventBatch, Kqueue},
    kqueue_change::{Action, Change, ChangeList, Filter},
    kqueue_policy::{
        delete_descriptor, exact, modify_descriptor, push_interests, register_descriptor,
    },
};

/// Fixed kqueue event storage.
#[derive(Debug)]
pub(crate) struct RawBatch {
    raw: KeventBatch,
}

impl RawBatch {
    fn new(capacity: usize) -> Option<Self> {
        KeventBatch::new(capacity).map(|raw| Self { raw })
    }

    pub(crate) fn event(&self, index: usize, observed: usize) -> Option<RawEvent> {
        let event = self.raw.event(index, observed)?;
        if event.filter() == Filter::User && event.token() == 0 {
            return Some(RawEvent::control());
        }
        let mut readiness = match event.filter() {
            Filter::Read => Readiness::READABLE,
            Filter::Write => Readiness::WRITABLE,
            Filter::User | Filter::Unknown => Readiness::ERROR,
        };
        if event.eof() {
            readiness = readiness.union(match event.filter() {
                Filter::Read => Readiness::READ_CLOSED,
                Filter::Write => Readiness::WRITE_CLOSED,
                Filter::User | Filter::Unknown => Readiness::ERROR,
            });
        }
        if event.error() {
            readiness = readiness.union(Readiness::ERROR);
        }
        Some(RawEvent::resource(event.token(), event.ident(), readiness))
    }
}

/// Clone-shared `EVFILT_USER` trigger for one kqueue.
#[derive(Debug)]
pub(crate) struct Wake {
    queue: Arc<Kqueue>,
}

impl Wake {
    pub(crate) fn wake(&self) -> io::Result<()> {
        let mut changes = ChangeList::new();
        changes
            .push(Change::new(0, Filter::User, Action::Trigger, 0))
            .ok_or_else(|| io::Error::other("kqueue wake change overflowed"))?;
        exact(&*self.queue, &changes, false)
    }
}

/// Kqueue selector with receipt-checked mutations.
#[derive(Debug)]
pub(crate) struct Backend {
    queue: Arc<Kqueue>,
}

impl Backend {
    pub(crate) fn raw_batch(capacity: usize) -> Option<RawBatch> {
        RawBatch::new(capacity)
    }

    pub(crate) fn new() -> Result<(Self, Arc<Wake>), SetupFailure> {
        let queue = Arc::new(
            Kqueue::new().map_err(|source| SetupFailure::new(Operation::CreatePoller, source))?,
        );
        let mut changes = ChangeList::new();
        changes
            .push(Change::new(0, Filter::User, Action::AddUser, 0))
            .ok_or_else(|| io::Error::other("kqueue setup change overflowed"))
            .map_err(|source| SetupFailure::new(Operation::RegisterWaker, source))?;
        exact(&*queue, &changes, false)
            .map_err(|source| SetupFailure::new(Operation::RegisterWaker, source))?;
        let wake = Arc::new(Wake {
            queue: Arc::clone(&queue),
        });
        Ok((Self { queue }, wake))
    }

    pub(crate) fn register(
        &self,
        source: BorrowedFd<'_>,
        token: u64,
        interest: Interest,
        _mode: Mode,
    ) -> Result<(), MutationFailure> {
        validate(token, interest)?;
        register_descriptor(&*self.queue, source.as_raw_fd(), token, interest)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "rollback requires the complete prior state"
    )]
    pub(crate) fn modify(
        &self,
        source: BorrowedFd<'_>,
        token: u64,
        previous_interest: Interest,
        previous_mode: Mode,
        previous_arm: ArmState,
        desired_interest: Interest,
        desired_mode: Mode,
    ) -> Result<(), MutationFailure> {
        validate(token, desired_interest)?;
        modify_descriptor(
            &*self.queue,
            source.as_raw_fd(),
            token,
            previous_interest,
            previous_mode,
            previous_arm,
            desired_interest,
            desired_mode,
        )
    }

    pub(crate) fn delete(
        &self,
        source: BorrowedFd<'_>,
        _interest: Interest,
    ) -> Result<(), MutationFailure> {
        delete_descriptor(&*self.queue, source.as_raw_fd())
    }

    pub(crate) fn wait(&self, batch: &mut RawBatch, wait: Wait) -> io::Result<usize> {
        self.queue.wait(&mut batch.raw, wait.timeout())
    }

    pub(crate) fn disarm(
        &self,
        descriptor: RawFd,
        interest: Interest,
    ) -> Result<(), MutationFailure> {
        if interest.is_empty() {
            return Err(invalid_mutation("cannot disarm empty interest"));
        }
        let mut changes = ChangeList::new();
        push_interests(&mut changes, descriptor, 0, interest, Action::Disable);
        exact(&*self.queue, &changes, false)
            .map_err(|source| MutationFailure::new(CommitStatus::Unknown, source))
    }
}

fn validate(token: u64, interest: Interest) -> Result<(), MutationFailure> {
    if token == 0 || interest.is_empty() {
        Err(invalid_mutation(
            "registration token and interest must be nonzero",
        ))
    } else {
        Ok(())
    }
}

fn invalid_mutation(message: &'static str) -> MutationFailure {
    MutationFailure::new(
        CommitStatus::NotApplied,
        io::Error::new(io::ErrorKind::InvalidInput, message),
    )
}
