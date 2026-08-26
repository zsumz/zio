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
    AlreadyAbsent,
    NotApplied(i32),
    Unknown(Option<i32>),
}

/// Executor seam shared by the native backend and deterministic policy tests.
pub(super) trait DisarmExecutor {
    fn apply(&mut self, changes: DisarmChanges<'_>) -> NativeApply;

    fn receipt(&self, index: usize, returned: usize, expected: Change) -> FilterApply;
}

#[derive(Clone, Copy, Debug)]
struct Plan {
    registration: RegistrationId,
    descriptor: RawFd,
    interest: Interest,
    commit: CommitStatus,
}

impl Plan {
    fn change_count(self) -> usize {
        usize::from(self.interest.is_readable()) + usize::from(self.interest.is_writable())
    }

    fn change(self, index: usize) -> Option<Change> {
        let filter = match (
            self.interest.is_readable(),
            self.interest.is_writable(),
            index,
        ) {
            (true, _, 0) => Filter::Read,
            (false, true, 0) | (true, true, 1) => Filter::Write,
            _ => return None,
        };
        Some(Change::new(self.descriptor, filter, Action::Disable, 0))
    }

    fn outcome(self) -> RecoveryOutcome {
        RecoveryOutcome::new(self.registration, self.commit)
    }
}

/// Allocation-free native-change traversal over retained logical plans.
#[derive(Clone)]
pub(super) struct DisarmChanges<'a> {
    plans: &'a [Plan],
    plan: usize,
    change: usize,
    remaining: usize,
}

impl Iterator for DisarmChanges<'_> {
    type Item = Change;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(plan) = self.plans.get(self.plan).copied() {
            if let Some(change) = plan.change(self.change) {
                self.change += 1;
                self.remaining -= 1;
                return Some(change);
            }
            self.plan += 1;
            self.change = 0;
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for DisarmChanges<'_> {
    fn len(&self) -> usize {
        self.remaining
    }
}

#[derive(Debug)]
pub(super) struct DisarmBatch {
    plan_capacity: usize,
    change_capacity: usize,
    change_len: usize,
    plans: Vec<Plan>,
}

impl DisarmBatch {
    pub(super) fn new(registrations: usize) -> Option<Self> {
        let changes = registrations.checked_mul(2)?;
        Some(Self {
            plan_capacity: registrations,
            change_capacity: changes,
            change_len: 0,
            plans: reserved(registrations)?,
        })
    }

    pub(super) fn clear(&mut self) {
        self.plans.clear();
        self.change_len = 0;
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
            || self.change_len.checked_add(required)? > self.change_capacity
        {
            return None;
        }
        self.plans.push(Plan {
            registration,
            descriptor,
            interest,
            commit: CommitStatus::Unknown,
        });
        self.change_len += required;
        Some(())
    }

    pub(super) fn submit<E: DisarmExecutor>(&mut self, executor: &mut E) -> io::Result<()> {
        if self.plans.is_empty() {
            return Ok(());
        }
        match executor.apply(self.changes()) {
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

    pub(super) fn outcomes(&self) -> impl Clone + ExactSizeIterator<Item = RecoveryOutcome> + '_ {
        self.plans.iter().copied().map(Plan::outcome)
    }

    #[cfg(test)]
    pub(super) fn storage_identity(&self) -> (usize, usize) {
        (self.plans.as_ptr() as usize, self.plans.capacity())
    }

    fn fill_outcomes(&mut self, commit: CommitStatus) {
        for plan in &mut self.plans {
            plan.commit = commit;
        }
    }

    fn changes(&self) -> DisarmChanges<'_> {
        DisarmChanges {
            plans: &self.plans,
            plan: 0,
            change: 0,
            remaining: self.change_len,
        }
    }

    fn reduce_receipts<E: DisarmExecutor>(
        &mut self,
        executor: &E,
        returned: usize,
    ) -> io::Result<()> {
        if returned > self.change_len {
            self.fill_outcomes(CommitStatus::Unknown);
            return Err(protocol_error());
        }
        let mut first_error = None;
        let mut change_index = 0;
        for plan in &mut self.plans {
            let mut satisfied = 0;
            let mut not_applied = 0;
            let mut unknown = false;
            let change_count = plan.change_count();
            for plan_index in 0..change_count {
                let expected = plan.change(plan_index).ok_or_else(protocol_error)?;
                match executor.receipt(change_index, returned, expected) {
                    FilterApply::Applied | FilterApply::AlreadyAbsent => satisfied += 1,
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
                change_index += 1;
            }
            plan.commit = if !unknown && satisfied == change_count {
                CommitStatus::Applied
            } else if !unknown && not_applied == change_count {
                CommitStatus::NotApplied
            } else {
                CommitStatus::Unknown
            };
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
