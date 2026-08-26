//! Registration-specialization rollback and error-preservation tests.

use std::{cell::Cell, io};

use crate::{CommitStatus, Interest, sys::MutationFailure};

use super::{
    kqueue_change::{Action, Change, ChangeList, Filter, Receipt, Receipts},
    kqueue_codec::encode_registration_change,
    kqueue_policy::{ChangeExecutor, register_descriptor},
};

#[test]
fn specialized_registration_change_omits_receipt_flag() -> io::Result<()> {
    let change = encode_registration_change(Change::new(17, Filter::Read, Action::AddEnabled, 41))?;

    assert!(!change.requests_receipt());
    Ok(())
}

#[derive(Debug)]
struct FastScript {
    registration_failure: Option<io::ErrorKind>,
    cleanup_failure: Option<io::ErrorKind>,
    cleanup_calls: Cell<usize>,
    cleanup: std::cell::RefCell<Vec<Change>>,
}

impl FastScript {
    fn new(
        registration_failure: Option<io::ErrorKind>,
        cleanup_failure: Option<io::ErrorKind>,
    ) -> Self {
        Self {
            registration_failure,
            cleanup_failure,
            cleanup_calls: Cell::new(0),
            cleanup: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl ChangeExecutor for FastScript {
    fn apply(&self, changes: &ChangeList) -> io::Result<Receipts> {
        self.cleanup_calls
            .set(self.cleanup_calls.get().saturating_add(1));
        self.cleanup
            .borrow_mut()
            .extend_from_slice(changes.as_slice());
        if let Some(kind) = self.cleanup_failure {
            return Err(io::Error::from(kind));
        }
        let mut receipts = Receipts::new(changes.as_slice().len());
        for (index, change) in changes.as_slice().iter().copied().enumerate() {
            receipts.set(index, Receipt::new(change.action(), None))?;
        }
        Ok(receipts)
    }

    fn apply_registration(
        &self,
        _descriptor: i32,
        _token: u64,
        _interest: Interest,
    ) -> io::Result<()> {
        self.registration_failure
            .map_or(Ok(()), |kind| Err(io::Error::from(kind)))
    }
}

#[test]
fn specialized_registration_success_skips_cleanup() -> io::Result<()> {
    let script = FastScript::new(None, Some(io::ErrorKind::Other));

    register_descriptor(&script, 17, 41, Interest::READABLE)
        .map_err(MutationFailure::into_source)?;

    assert_eq!(script.cleanup_calls.get(), 0);
    assert!(script.cleanup.borrow().is_empty());
    Ok(())
}

#[test]
fn specialized_registration_error_is_preserved_after_exact_cleanup() -> io::Result<()> {
    let script = FastScript::new(Some(io::ErrorKind::PermissionDenied), None);

    let failure = registration_failure(&script)?;

    assert_eq!(failure.commit(), CommitStatus::NotApplied);
    assert_eq!(
        failure.into_source().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(script.cleanup_calls.get(), 1);
    let cleanup = script.cleanup.borrow();
    assert_eq!(cleanup.len(), 2);
    assert_eq!(cleanup[0].filter(), Filter::Read);
    assert_eq!(cleanup[1].filter(), Filter::Write);
    assert!(
        cleanup
            .iter()
            .all(|change| change.action() == Action::Delete)
    );
    Ok(())
}

#[test]
fn specialized_registration_error_is_unknown_when_cleanup_fails() -> io::Result<()> {
    let script = FastScript::new(
        Some(io::ErrorKind::PermissionDenied),
        Some(io::ErrorKind::Other),
    );

    let failure = registration_failure(&script)?;

    assert_eq!(failure.commit(), CommitStatus::Unknown);
    assert_eq!(
        failure.into_source().kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(script.cleanup_calls.get(), 1);
    Ok(())
}

fn registration_failure(script: &FastScript) -> io::Result<MutationFailure> {
    register_descriptor(script, 19, 43, Interest::READABLE)
        .err()
        .ok_or_else(|| io::Error::other("injected registration unexpectedly succeeded"))
}
