//! Exact-state kqueue deletion scope and retry regressions.

use std::{cell::RefCell, collections::VecDeque, io};

use crate::{ArmState, CommitStatus, Interest, RegistrationState, sys::MutationFailure};

use super::{
    kqueue_change::{Action, Change, ChangeList, Filter, Receipt, Receipts},
    kqueue_policy::{ChangeExecutor, delete_descriptor},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Installed {
    read: bool,
    write: bool,
}

impl Installed {
    const BOTH: Self = Self {
        read: true,
        write: true,
    };

    const fn from_interest(interest: Interest) -> Self {
        Self {
            read: interest.is_readable(),
            write: interest.is_writable(),
        }
    }

    fn apply(&mut self, change: Change) {
        let target = match change.filter() {
            Filter::Read => &mut self.read,
            Filter::Write => &mut self.write,
            Filter::User | Filter::Unknown => return,
        };
        match change.action() {
            Action::AddEnabled | Action::AddDisabled => *target = true,
            Action::Delete => *target = false,
            Action::Disable | Action::AddUser | Action::Trigger => {}
        }
    }
}

#[derive(Debug)]
struct Script {
    installed: RefCell<Installed>,
    outcomes: RefCell<VecDeque<Vec<Result<(), i32>>>>,
    submissions: RefCell<Vec<Vec<Change>>>,
}

impl Script {
    fn new(installed: Installed, outcomes: impl IntoIterator<Item = Vec<Result<(), i32>>>) -> Self {
        Self {
            installed: RefCell::new(installed),
            outcomes: RefCell::new(outcomes.into_iter().collect()),
            submissions: RefCell::new(Vec::new()),
        }
    }

    fn installed(&self) -> Installed {
        *self.installed.borrow()
    }

    fn filters(&self, submission: usize) -> Vec<Filter> {
        self.submissions
            .borrow()
            .get(submission)
            .into_iter()
            .flatten()
            .map(|change| change.filter())
            .collect()
    }
}

impl ChangeExecutor for Script {
    fn apply(&self, changes: &ChangeList) -> io::Result<Receipts> {
        self.submissions
            .borrow_mut()
            .push(changes.as_slice().to_vec());
        let outcomes = self
            .outcomes
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| io::Error::other("kqueue delete script exhausted"))?;
        let mut receipts = Receipts::new(changes.as_slice().len());
        for (index, change) in changes.as_slice().iter().copied().enumerate() {
            let outcome = outcomes.get(index).copied().unwrap_or(Ok(()));
            if outcome.is_ok() {
                self.installed.borrow_mut().apply(change);
            }
            receipts.set(index, Receipt::new(change.action(), outcome.err()))?;
        }
        Ok(receipts)
    }
}

#[test]
fn registered_state_scopes_deletes_while_uncertain_state_sweeps() -> Result<(), io::Error> {
    let armed = registered(ArmState::Armed);
    let disarmed = registered(ArmState::Disarmed);
    verify_scope(Interest::READABLE, armed, &[Filter::Read])?;
    verify_scope(Interest::WRITABLE, armed, &[Filter::Write])?;
    verify_scope(Interest::READABLE, disarmed, &[Filter::Read])?;
    verify_scope(
        Interest::READABLE | Interest::WRITABLE,
        armed,
        &[Filter::Read, Filter::Write],
    )?;
    verify_scope(
        Interest::WRITABLE,
        RegistrationState::Uncertain,
        &[Filter::Read, Filter::Write],
    )
}

#[test]
fn failed_singleton_delete_becomes_unknown_and_retry_sweeps() -> Result<(), io::Error> {
    let script = Script::new(
        Installed::from_interest(Interest::READABLE),
        [vec![Err(libc::EIO)], vec![Ok(()), Err(libc::ENOENT)]],
    );

    let failure = delete_descriptor(&script, 19, Interest::READABLE, registered(ArmState::Armed))
        .err()
        .ok_or_else(|| io::Error::other("injected singleton delete unexpectedly succeeded"))?;
    assert_eq!(failure.commit(), CommitStatus::Unknown);
    assert_eq!(
        script.installed(),
        Installed::from_interest(Interest::READABLE)
    );

    delete_descriptor(
        &script,
        19,
        Interest::READABLE,
        RegistrationState::Uncertain,
    )
    .map_err(MutationFailure::into_source)?;
    assert_eq!(script.filters(0), [Filter::Read]);
    assert_eq!(script.filters(1), [Filter::Read, Filter::Write]);
    assert_eq!(script.installed(), Installed::default());
    Ok(())
}

#[test]
fn partial_combined_delete_becomes_unknown_and_retry_sweeps() -> Result<(), io::Error> {
    let combined = Interest::READABLE | Interest::WRITABLE;
    let script = Script::new(
        Installed::BOTH,
        [
            vec![Ok(()), Err(libc::EIO)],
            vec![Err(libc::ENOENT), Ok(())],
        ],
    );

    let failure = delete_descriptor(&script, 23, combined, registered(ArmState::Disarmed))
        .err()
        .ok_or_else(|| io::Error::other("partial combined delete unexpectedly succeeded"))?;
    assert_eq!(failure.commit(), CommitStatus::Unknown);
    assert_eq!(
        script.installed(),
        Installed {
            read: false,
            write: true
        }
    );

    delete_descriptor(&script, 23, combined, RegistrationState::Uncertain)
        .map_err(MutationFailure::into_source)?;
    assert_eq!(script.filters(0), [Filter::Read, Filter::Write]);
    assert_eq!(script.filters(1), [Filter::Read, Filter::Write]);
    assert_eq!(script.installed(), Installed::default());
    Ok(())
}

fn verify_scope(
    interest: Interest,
    state: RegistrationState,
    expected: &[Filter],
) -> Result<(), io::Error> {
    let installed = match state {
        RegistrationState::Registered { .. } => Installed::from_interest(interest),
        RegistrationState::Uncertain => Installed::BOTH,
    };
    let script = Script::new(installed, [vec![Ok(()); expected.len()]]);
    delete_descriptor(&script, 17, interest, state).map_err(MutationFailure::into_source)?;
    assert_eq!(script.filters(0), expected);
    assert!(
        script.submissions.borrow()[0]
            .iter()
            .all(|change| change.action() == Action::Delete)
    );
    assert_eq!(script.installed(), Installed::default());
    Ok(())
}

const fn registered(arm: ArmState) -> RegistrationState {
    RegistrationState::Registered { arm }
}
