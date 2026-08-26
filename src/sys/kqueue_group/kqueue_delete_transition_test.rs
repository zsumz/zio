//! Register and modify invariants that justify scoped kqueue deletion.

use std::{cell::RefCell, collections::VecDeque, io};

use crate::{ArmState, CommitStatus, Interest, Mode, RegistrationState, sys::MutationFailure};

use super::{
    kqueue_change::{Action, Change, ChangeList, Filter, Receipt, Receipts},
    kqueue_policy::{ChangeExecutor, delete_descriptor, modify_descriptor, register_descriptor},
};

const TOKEN: u64 = 47;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct InstalledFilter {
    token: u64,
    enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Installed {
    read: Option<InstalledFilter>,
    write: Option<InstalledFilter>,
}

impl Installed {
    fn exact(interest: Interest, arm: ArmState) -> Self {
        let filter = Some(InstalledFilter {
            token: TOKEN,
            enabled: arm == ArmState::Armed,
        });
        Self {
            read: interest.is_readable().then_some(filter).flatten(),
            write: interest.is_writable().then_some(filter).flatten(),
        }
    }

    fn apply(&mut self, change: Change) {
        let target = match change.filter() {
            Filter::Read => &mut self.read,
            Filter::Write => &mut self.write,
            Filter::User | Filter::Unknown => return,
        };
        match change.action() {
            Action::AddEnabled => {
                *target = Some(InstalledFilter {
                    token: change.token(),
                    enabled: true,
                });
            }
            Action::AddDisabled => {
                *target = Some(InstalledFilter {
                    token: change.token(),
                    enabled: false,
                });
            }
            Action::Delete => *target = None,
            Action::Disable => {
                if let Some(filter) = target {
                    filter.enabled = false;
                }
            }
            Action::AddUser | Action::Trigger => {}
        }
    }
}

#[derive(Debug)]
struct Model {
    installed: RefCell<Installed>,
    outcomes: RefCell<VecDeque<Vec<Result<(), i32>>>>,
    submissions: RefCell<Vec<Vec<Change>>>,
}

impl Model {
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
        self.submissions.borrow()[submission]
            .iter()
            .map(|change| change.filter())
            .collect()
    }
}

impl ChangeExecutor for Model {
    fn apply(&self, changes: &ChangeList) -> io::Result<Receipts> {
        self.submissions
            .borrow_mut()
            .push(changes.as_slice().to_vec());
        let outcomes = self.outcomes.borrow_mut().pop_front().unwrap_or_default();
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
fn failed_single_interest_register_still_sweeps_both_filters() -> io::Result<()> {
    let model = Model::new(
        Installed::default(),
        [
            vec![Err(libc::EIO)],
            vec![Err(libc::ENOENT), Err(libc::ENOENT)],
        ],
    );

    let failure = register_descriptor(&model, 29, TOKEN, Interest::READABLE)
        .err()
        .ok_or_else(|| io::Error::other("injected registration unexpectedly succeeded"))?;

    assert_eq!(failure.commit(), CommitStatus::NotApplied);
    assert_eq!(model.filters(0), [Filter::Read]);
    assert_eq!(model.filters(1), [Filter::Read, Filter::Write]);
    assert_eq!(model.installed(), Installed::default());
    Ok(())
}

#[test]
fn every_successful_interest_transition_leaves_exact_deletable_state() -> io::Result<()> {
    let interests = [
        Interest::READABLE,
        Interest::WRITABLE,
        Interest::READABLE | Interest::WRITABLE,
    ];
    for arm in [ArmState::Armed, ArmState::Disarmed] {
        for prior in interests {
            for desired in interests {
                verify_transition(prior, arm, desired)?;
            }
        }
    }
    Ok(())
}

#[test]
fn successful_rollback_restores_exact_scoped_delete_state() -> io::Result<()> {
    let prior = Interest::READABLE;
    let desired = Interest::READABLE | Interest::WRITABLE;
    let model = Model::new(
        Installed::exact(prior, ArmState::Disarmed),
        [vec![Err(libc::EIO), Ok(())], vec![Ok(()), Ok(())]],
    );

    let failure = modify_descriptor(
        &model,
        31,
        TOKEN,
        prior,
        Mode::OneShot,
        ArmState::Disarmed,
        desired,
        Mode::Level,
    )
    .err()
    .ok_or_else(|| io::Error::other("injected modification unexpectedly succeeded"))?;
    assert_eq!(failure.commit(), CommitStatus::NotApplied);
    assert_eq!(
        model.installed(),
        Installed::exact(prior, ArmState::Disarmed)
    );

    delete_descriptor(
        &model,
        31,
        prior,
        RegistrationState::Registered {
            arm: ArmState::Disarmed,
        },
    )
    .map_err(MutationFailure::into_source)?;
    assert_eq!(model.filters(2), [Filter::Read]);
    assert_eq!(model.installed(), Installed::default());
    Ok(())
}

fn verify_transition(prior: Interest, arm: ArmState, desired: Interest) -> io::Result<()> {
    let model = Model::new(Installed::exact(prior, arm), []);
    modify_descriptor(
        &model,
        37,
        TOKEN,
        prior,
        Mode::OneShot,
        arm,
        desired,
        Mode::Level,
    )
    .map_err(MutationFailure::into_source)?;
    assert_eq!(
        model.installed(),
        Installed::exact(desired, ArmState::Armed)
    );

    delete_descriptor(
        &model,
        37,
        desired,
        RegistrationState::Registered {
            arm: ArmState::Armed,
        },
    )
    .map_err(MutationFailure::into_source)?;
    assert_eq!(model.installed(), Installed::default());
    Ok(())
}
