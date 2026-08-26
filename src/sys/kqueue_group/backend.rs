//! Safe receipt-checked kqueue backend and event normalization.

use std::{
    io,
    os::fd::{AsRawFd, BorrowedFd, RawFd},
    sync::Arc,
};

use crate::{
    ArmState, Interest, Mode, Readiness, RecoveryOutcome, RegistrationId, RegistrationState, Wait,
    error::Operation,
};

use super::super::{
    event::RawEvent,
    failure::{MutationFailure, SetupFailure},
};
use super::{
    kqueue::{KeventBatch, Kqueue},
    kqueue_change::{Action, Change, ChangeList, Filter},
    kqueue_disarm::{DisarmBatch, DisarmChanges, DisarmExecutor, FilterApply, NativeApply},
    kqueue_policy::{delete_descriptor, exact, modify_descriptor, register_descriptor},
};

/// Fixed kqueue event storage.
#[derive(Debug)]
pub(crate) struct RawBatch {
    raw: KeventBatch,
    disarms: DisarmBatch,
}

impl RawBatch {
    fn new(event_capacity: usize, disarm_capacity: usize) -> Option<Self> {
        let native_capacity = disarm_capacity.checked_mul(2)?;
        Some(Self {
            raw: KeventBatch::new(event_capacity, native_capacity)?,
            disarms: DisarmBatch::new(disarm_capacity)?,
        })
    }

    pub(crate) fn event(&self, index: usize, observed: usize) -> Option<RawEvent> {
        let event = self.raw.event(index, observed)?;
        if event.filter() == Filter::User && event.token() == 0 {
            return Some(RawEvent::control());
        }
        Some(RawEvent::resource(
            event.token(),
            event.ident(),
            from_kqueue_event(event),
        ))
    }

    pub(crate) fn clear_disarms(&mut self) {
        self.disarms.clear();
    }

    pub(crate) fn push_disarm(
        &mut self,
        registration: RegistrationId,
        descriptor: RawFd,
        interest: Interest,
    ) -> Option<()> {
        self.disarms.push(registration, descriptor, interest)
    }

    pub(crate) fn disarm_outcomes(
        &self,
    ) -> impl Clone + ExactSizeIterator<Item = RecoveryOutcome> + '_ {
        self.disarms.outcomes()
    }
}

pub(super) fn from_kqueue_event(event: super::kqueue_change::RawKevent) -> Readiness {
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
    if event.native_error() || (event.eof() && event.fflags() != 0) {
        readiness = readiness.union(Readiness::ERROR);
    }
    readiness
}

/// Clone-shared `EVFILT_USER` trigger for one kqueue.
#[derive(Clone, Debug)]
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
    pub(crate) fn raw_batch(event_capacity: usize, disarm_capacity: usize) -> Option<RawBatch> {
        RawBatch::new(event_capacity, disarm_capacity)
    }

    pub(crate) fn new() -> Result<(Self, Wake), SetupFailure> {
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
        let wake = Wake {
            queue: Arc::clone(&queue),
        };
        Ok((Self { queue }, wake))
    }

    pub(crate) fn register(
        &self,
        source: BorrowedFd<'_>,
        token: u64,
        interest: Interest,
        _mode: Mode,
    ) -> Result<(), MutationFailure> {
        debug_assert_ne!(token, 0);
        debug_assert!(!interest.is_empty());
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
        debug_assert_ne!(token, 0);
        debug_assert!(!desired_interest.is_empty());
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
        interest: Interest,
        state: RegistrationState,
    ) -> Result<(), MutationFailure> {
        delete_descriptor(&*self.queue, source.as_raw_fd(), interest, state)
    }

    pub(crate) fn wait(&self, batch: &mut RawBatch, wait: Wait) -> io::Result<usize> {
        self.queue.wait(&mut batch.raw, wait.timeout())
    }

    pub(crate) fn submit_disarms(&self, batch: &mut RawBatch) -> io::Result<()> {
        let mut executor = NativeDisarmExecutor {
            queue: &self.queue,
            raw: &mut batch.raw,
        };
        batch.disarms.submit(&mut executor)
    }
}

struct NativeDisarmExecutor<'a> {
    queue: &'a Kqueue,
    raw: &'a mut KeventBatch,
}

impl DisarmExecutor for NativeDisarmExecutor<'_> {
    fn apply(&mut self, changes: DisarmChanges<'_>) -> NativeApply {
        self.queue.apply_batch(changes, self.raw)
    }

    fn receipt(&self, index: usize, returned: usize, expected: Change) -> FilterApply {
        self.raw.receipt(index, returned, expected)
    }
}
