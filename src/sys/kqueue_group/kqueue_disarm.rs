//! Bounded one-shot disarm batching and logical outcome reduction.

use std::{io, os::fd::RawFd};

use crate::{CommitStatus, Interest, RecoveryOutcome, RegistrationId};

use super::kqueue_change::{Action, Change, Filter};

/// Result of one native changelist submission.
pub(super) enum NativeApply {
    Receipts(usize),
    AppliedWithoutReceipts,
    Unknown(io::Error),
}

/// Proven result of one native filter change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FilterApply {
    Applied,
    NotApplied(i32),
    Unknown(Option<i32>),
}

/// Executor seam shared by the native backend and deterministic policy tests.
pub(super) trait DisarmExecutor {
    fn apply(&mut self, changes: &[Change]) -> NativeApply;

    fn receipt(&self, index: usize, returned: usize, expected: Change) -> FilterApply;
}

#[derive(Clone, Copy, Debug)]
struct Plan {
    registration: RegistrationId,
    first: usize,
    len: usize,
}

/// Construction-time storage for one logical disarm batch.
#[derive(Debug)]
pub(super) struct DisarmBatch {
    plan_capacity: usize,
    change_capacity: usize,
    plans: Vec<Plan>,
    changes: Vec<Change>,
    outcomes: Vec<RecoveryOutcome>,
}

impl DisarmBatch {
    pub(super) fn new(registrations: usize) -> Option<Self> {
        let changes = registrations.checked_mul(2)?;
        Some(Self {
            plan_capacity: registrations,
            change_capacity: changes,
            plans: reserved(registrations)?,
            changes: reserved(changes)?,
            outcomes: reserved(registrations)?,
        })
    }

    pub(super) fn clear(&mut self) {
        self.plans.clear();
        self.changes.clear();
        self.outcomes.clear();
    }

    pub(super) fn push(
        &mut self,
        registration: RegistrationId,
        descriptor: RawFd,
        interest: Interest,
    ) -> Option<()> {
        let required = usize::from(interest.is_readable()) + usize::from(interest.is_writable());
        if required == 0
            || self.plans.len() >= self.plan_capacity
            || self.changes.len().checked_add(required)? > self.change_capacity
        {
            return None;
        }
        let first = self.changes.len();
        if interest.is_readable() {
            self.changes
                .push(Change::new(descriptor, Filter::Read, Action::Disable, 0));
        }
        if interest.is_writable() {
            self.changes
                .push(Change::new(descriptor, Filter::Write, Action::Disable, 0));
        }
        self.plans.push(Plan {
            registration,
            first,
            len: required,
        });
        Some(())
    }

    pub(super) fn submit<E: DisarmExecutor>(&mut self, executor: &mut E) -> io::Result<()> {
        self.outcomes.clear();
        if self.changes.is_empty() {
            return Ok(());
        }
        match executor.apply(&self.changes) {
            NativeApply::AppliedWithoutReceipts => {
                self.fill_outcomes(CommitStatus::Applied);
                Ok(())
            }
            NativeApply::Unknown(source) => {
                self.fill_outcomes(CommitStatus::Unknown);
                Err(source)
            }
            NativeApply::Receipts(returned) => self.reduce_receipts(executor, returned),
        }
    }

    pub(super) fn outcomes(&self) -> &[RecoveryOutcome] {
        &self.outcomes
    }

    #[cfg(test)]
    pub(super) fn storage_identity(&self) -> [(usize, usize); 3] {
        [
            (self.plans.as_ptr() as usize, self.plans.capacity()),
            (self.changes.as_ptr() as usize, self.changes.capacity()),
            (self.outcomes.as_ptr() as usize, self.outcomes.capacity()),
        ]
    }

    fn fill_outcomes(&mut self, commit: CommitStatus) {
        for plan in &self.plans {
            self.outcomes
                .push(RecoveryOutcome::new(plan.registration, commit));
        }
    }

    fn reduce_receipts<E: DisarmExecutor>(
        &mut self,
        executor: &E,
        returned: usize,
    ) -> io::Result<()> {
        if returned > self.changes.len() {
            self.fill_outcomes(CommitStatus::Unknown);
            return Err(protocol_error());
        }
        let mut first_error = None;
        for plan in &self.plans {
            let mut applied = 0;
            let mut not_applied = 0;
            let mut unknown = false;
            for index in plan.first..plan.first + plan.len {
                match executor.receipt(index, returned, self.changes[index]) {
                    FilterApply::Applied => applied += 1,
                    FilterApply::NotApplied(code) => {
                        not_applied += 1;
                        first_error.get_or_insert_with(|| io::Error::from_raw_os_error(code));
                    }
                    FilterApply::Unknown(code) => {
                        unknown = true;
                        first_error.get_or_insert_with(|| {
                            code.map_or_else(protocol_error, io::Error::from_raw_os_error)
                        });
                    }
                }
            }
            let commit = if !unknown && applied == plan.len {
                CommitStatus::Applied
            } else if !unknown && not_applied == plan.len {
                CommitStatus::NotApplied
            } else {
                CommitStatus::Unknown
            };
            self.outcomes
                .push(RecoveryOutcome::new(plan.registration, commit));
        }
        first_error.map_or(Ok(()), Err)
    }
}

fn reserved<T>(capacity: usize) -> Option<Vec<T>> {
    let mut items = Vec::new();
    items.try_reserve_exact(capacity).ok()?;
    Some(items)
}

fn protocol_error() -> io::Error {
    io::Error::from(io::ErrorKind::InvalidData)
}
