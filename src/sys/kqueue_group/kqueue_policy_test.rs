//! Stateful tests for exact kqueue rollback claims.

use std::{cell::RefCell, collections::VecDeque, io};

use crate::{ArmState, Interest, Mode, error::CommitStatus};

use super::{
    kqueue_change::{Action, Change, ChangeList, Filter, Receipt, Receipts},
    kqueue_policy::{ChangeExecutor, delete_descriptor, modify_descriptor, register_descriptor},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InstalledFilter {
    token: u64,
    enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct InstalledFilters {
    read: Option<InstalledFilter>,
    write: Option<InstalledFilter>,
}

impl InstalledFilters {
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
                if let Some(installed) = target {
                    installed.enabled = false;
                }
            }
            Action::AddUser | Action::Trigger => {}
        }
    }
}

#[derive(Debug)]
struct Script {
    installed: RefCell<InstalledFilters>,
    outcomes: RefCell<VecDeque<Vec<Result<(), i32>>>>,
}

impl Script {
    fn new(
        installed: InstalledFilters,
        outcomes: impl IntoIterator<Item = Vec<Result<(), i32>>>,
    ) -> Self {
        Self {
            installed: RefCell::new(installed),
            outcomes: RefCell::new(outcomes.into_iter().collect()),
        }
    }

    fn installed(&self) -> InstalledFilters {
        *self.installed.borrow()
    }
}

impl ChangeExecutor for Script {
    fn apply(&self, changes: &ChangeList) -> io::Result<Receipts> {
        let outcomes = self
            .outcomes
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| io::Error::other("kqueue test script exhausted"))?;
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
fn failed_modify_restores_disarmed_prior_filters() {
    const TOKEN: u64 = 17;
    let prior = InstalledFilters {
        read: Some(InstalledFilter {
            token: TOKEN,
            enabled: false,
        }),
        write: None,
    };
    let script = Script::new(prior, [vec![Err(libc::EIO), Ok(())], vec![Ok(()), Ok(())]]);

    let result = modify_descriptor(
        &script,
        5,
        TOKEN,
        Interest::READABLE,
        Mode::OneShot,
        ArmState::Disarmed,
        Interest::READABLE.union(Interest::WRITABLE),
        Mode::OneShot,
    );

    assert_eq!(
        result.err().map(|failure| failure.commit()),
        Some(CommitStatus::NotApplied)
    );
    assert_eq!(script.installed(), prior);
}

#[test]
fn failed_modify_with_failed_restore_is_unknown() {
    const TOKEN: u64 = 23;
    let prior = InstalledFilters {
        read: Some(InstalledFilter {
            token: TOKEN,
            enabled: false,
        }),
        write: None,
    };
    let script = Script::new(
        prior,
        [vec![Err(libc::EIO), Ok(())], vec![Ok(()), Err(libc::EIO)]],
    );

    let result = modify_descriptor(
        &script,
        7,
        TOKEN,
        Interest::READABLE,
        Mode::OneShot,
        ArmState::Disarmed,
        Interest::READABLE.union(Interest::WRITABLE),
        Mode::Level,
    );

    assert_eq!(
        result.err().map(|failure| failure.commit()),
        Some(CommitStatus::Unknown)
    );
    assert_ne!(script.installed(), prior);
}

#[test]
fn partial_register_failure_with_cleanup_is_not_applied() {
    const TOKEN: u64 = 31;
    let script = Script::new(
        InstalledFilters::default(),
        [
            vec![Ok(()), Err(libc::EIO)],
            vec![Ok(()), Err(libc::ENOENT)],
        ],
    );

    let result = register_descriptor(
        &script,
        9,
        TOKEN,
        Interest::READABLE.union(Interest::WRITABLE),
    );

    assert_eq!(
        result.err().map(|failure| failure.commit()),
        Some(CommitStatus::NotApplied)
    );
    assert_eq!(script.installed(), InstalledFilters::default());
}

#[test]
fn partial_register_failure_with_failed_cleanup_is_unknown() {
    const TOKEN: u64 = 37;
    let script = Script::new(
        InstalledFilters::default(),
        [
            vec![Ok(()), Err(libc::EIO)],
            vec![Err(libc::EIO), Err(libc::ENOENT)],
        ],
    );

    let result = register_descriptor(
        &script,
        11,
        TOKEN,
        Interest::READABLE.union(Interest::WRITABLE),
    );

    assert_eq!(
        result.err().map(|failure| failure.commit()),
        Some(CommitStatus::Unknown)
    );
    assert_eq!(
        script.installed().read,
        Some(InstalledFilter {
            token: TOKEN,
            enabled: true,
        })
    );
}

#[test]
fn failed_delete_cleanup_is_unknown_and_retains_failed_filter() {
    const TOKEN: u64 = 41;
    let prior = InstalledFilters {
        read: Some(InstalledFilter {
            token: TOKEN,
            enabled: true,
        }),
        write: Some(InstalledFilter {
            token: TOKEN,
            enabled: true,
        }),
    };
    let script = Script::new(prior, [vec![Ok(()), Err(libc::EIO)]]);

    let result = delete_descriptor(&script, 13);

    assert_eq!(
        result.err().map(|failure| failure.commit()),
        Some(CommitStatus::Unknown)
    );
    assert_eq!(script.installed().read, None);
    assert_eq!(script.installed().write, prior.write);
}
