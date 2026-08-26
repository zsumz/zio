//! Deterministic policy tests for retained one-shot disarm batches.

use std::{io, os::fd::AsRawFd, os::unix::net::UnixStream};

use crate::{CommitStatus, Interest, RegistrationId};

use super::{
    kqueue::{KeventBatch, Kqueue},
    kqueue_change::{Action, Change, Filter},
    kqueue_codec::{classify_apply_error, missing_entry_error_code},
    kqueue_disarm::{DisarmBatch, DisarmChanges, DisarmExecutor, FilterApply, NativeApply},
};

#[derive(Clone, Copy, Debug)]
enum ScriptApply {
    Receipts(usize),
    AppliedWithoutReceipts,
    Unknown,
}

#[derive(Debug)]
struct Script {
    apply: ScriptApply,
    receipts: Vec<FilterApply>,
    submissions: usize,
}

impl Script {
    fn receipts(receipts: impl IntoIterator<Item = FilterApply>) -> Self {
        let receipts = receipts.into_iter().collect::<Vec<_>>();
        Self {
            apply: ScriptApply::Receipts(receipts.len()),
            receipts,
            submissions: 0,
        }
    }
}

impl DisarmExecutor for Script {
    fn apply(&mut self, _changes: DisarmChanges<'_>) -> NativeApply {
        self.submissions += 1;
        match self.apply {
            ScriptApply::Receipts(returned) => NativeApply::Receipts(returned),
            ScriptApply::AppliedWithoutReceipts => NativeApply::AppliedWithoutReceipts,
            ScriptApply::Unknown => {
                NativeApply::Unknown(io::Error::from(io::ErrorKind::PermissionDenied))
            }
        }
    }

    fn receipt(&self, index: usize, returned: usize, _expected: Change) -> FilterApply {
        if index >= returned {
            FilterApply::Unknown(None)
        } else {
            self.receipts
                .get(index)
                .copied()
                .unwrap_or(FilterApply::Unknown(None))
        }
    }
}

#[test]
fn one_submission_reduces_multiple_registrations() {
    let Some(mut batch) = populated_batch() else {
        return;
    };
    let mut script = Script::receipts([
        FilterApply::Applied,
        FilterApply::AlreadyAbsent,
        FilterApply::Applied,
    ]);

    assert!(batch.submit(&mut script).is_ok());

    assert_eq!(script.submissions, 1);
    assert_statuses(&batch, &[CommitStatus::Applied, CommitStatus::Applied]);
}

#[test]
fn all_failed_filters_preserve_armed_outcome() {
    let Some(mut batch) = two_filter_batch() else {
        return;
    };
    let mut script = Script::receipts([FilterApply::NotApplied(13), FilterApply::NotApplied(13)]);

    let result = batch.submit(&mut script);
    assert!(result.is_err());
    let Some(error) = result.err() else {
        return;
    };

    assert_eq!(error.raw_os_error(), Some(13));
    assert_statuses(&batch, &[CommitStatus::NotApplied]);
}

#[test]
fn mixed_filter_results_are_unknown() {
    let Some(mut batch) = two_filter_batch() else {
        return;
    };
    let mut script = Script::receipts([FilterApply::AlreadyAbsent, FilterApply::NotApplied(5)]);

    assert!(batch.submit(&mut script).is_err());

    assert_statuses(&batch, &[CommitStatus::Unknown]);
}

#[test]
fn uncertain_receipt_preserves_native_error() {
    let Some(mut batch) = two_filter_batch() else {
        return;
    };
    let mut script = Script::receipts([FilterApply::Unknown(Some(73)), FilterApply::Applied]);

    let Some(error) = batch.submit(&mut script).err() else {
        return;
    };
    assert_eq!(error.raw_os_error(), Some(73));
    assert_statuses(&batch, &[CommitStatus::Unknown]);
}

#[test]
fn short_receipt_sets_are_unknown() {
    let Some(mut batch) = two_filter_batch() else {
        return;
    };
    let mut script = Script {
        apply: ScriptApply::Receipts(1),
        receipts: vec![FilterApply::Applied],
        submissions: 0,
    };

    assert!(batch.submit(&mut script).is_err());

    assert_statuses(&batch, &[CommitStatus::Unknown]);
}

#[test]
fn malformed_receipts_are_unknown() {
    let expected = read_change(5);
    let native = KeventBatch::new(1, 1);
    assert!(native.is_some());
    let Some(mut native) = native else {
        return;
    };
    assert!(native.stage_receipt(0, expected, 0, false).is_some());
    assert_eq!(native.receipt(0, 1, expected), FilterApply::Unknown(None));
}

#[test]
fn mismatched_receipts_are_unknown() {
    let expected = read_change(5);
    let native = KeventBatch::new(1, 1);
    assert!(native.is_some());
    let Some(mut native) = native else {
        return;
    };
    assert!(native.stage_receipt(0, read_change(7), 0, true).is_some());
    assert_eq!(native.receipt(0, 1, expected), FilterApply::Unknown(None));
}

#[test]
fn missing_native_disable_is_classified_as_already_absent() -> io::Result<()> {
    let (source, _peer) = UnixStream::pair()?;
    let queue = Kqueue::new()?;
    let expected = read_change(source.as_raw_fd());
    let mut native = KeventBatch::new(1, 1)
        .ok_or_else(|| io::Error::other("native receipt storage unavailable"))?;

    assert!(matches!(
        queue.apply_batch([expected].into_iter(), &mut native),
        NativeApply::Receipts(1)
    ));
    assert_eq!(native.receipt(0, 1, expected), FilterApply::AlreadyAbsent);
    let addition = Change::new(source.as_raw_fd(), Filter::Read, Action::AddEnabled, 0);
    let missing_entry = missing_entry_error_code();
    assert!(
        native
            .stage_receipt(0, addition, missing_entry, true)
            .is_some()
    );
    assert_eq!(
        native.receipt(0, 1, addition),
        FilterApply::NotApplied(missing_entry)
    );
    Ok(())
}

#[test]
fn syscall_failure_is_unknown() {
    let Some(mut batch) = populated_batch() else {
        return;
    };
    let mut script = Script {
        apply: ScriptApply::Unknown,
        receipts: Vec::new(),
        submissions: 0,
    };

    assert!(batch.submit(&mut script).is_err());

    assert_statuses(&batch, &[CommitStatus::Unknown, CommitStatus::Unknown]);
}

#[test]
fn freebsd_and_netbsd_interruptions_are_applied() {
    let result = classify_apply_error(io::Error::from(io::ErrorKind::Interrupted), true);
    assert!(matches!(result, NativeApply::AppliedWithoutReceipts));

    let Some(mut batch) = populated_batch() else {
        return;
    };
    let mut script = Script {
        apply: ScriptApply::AppliedWithoutReceipts,
        receipts: Vec::new(),
        submissions: 0,
    };
    assert!(batch.submit(&mut script).is_ok());
    assert_statuses(&batch, &[CommitStatus::Applied, CommitStatus::Applied]);
}

#[test]
fn macos_interruptions_are_unknown() {
    let result = classify_apply_error(io::Error::from(io::ErrorKind::Interrupted), false);
    assert!(matches!(result, NativeApply::Unknown(_)));
}

#[test]
fn successful_batches_reuse_retained_storage() {
    let batch = DisarmBatch::new(2);
    assert!(batch.is_some());
    let Some(mut batch) = batch else {
        return;
    };
    let retained = batch.storage_identity();
    for receipt in [FilterApply::Applied, FilterApply::AlreadyAbsent] {
        assert!(
            batch
                .push(RegistrationId::new(1), 5, Interest::READABLE)
                .is_some()
        );
        let mut script = Script::receipts([receipt]);
        assert!(batch.submit(&mut script).is_ok());
        batch.clear();
        assert_eq!(batch.storage_identity(), retained);
    }
}

fn populated_batch() -> Option<DisarmBatch> {
    let batch = DisarmBatch::new(2);
    assert!(batch.is_some());
    let mut batch = batch?;
    assert!(
        batch
            .push(
                RegistrationId::new(1),
                5,
                Interest::READABLE | Interest::WRITABLE,
            )
            .is_some()
    );
    assert!(
        batch
            .push(RegistrationId::new(2), 7, Interest::READABLE)
            .is_some()
    );
    Some(batch)
}

fn two_filter_batch() -> Option<DisarmBatch> {
    let batch = DisarmBatch::new(1);
    assert!(batch.is_some());
    let mut batch = batch?;
    assert!(
        batch
            .push(
                RegistrationId::new(1),
                5,
                Interest::READABLE | Interest::WRITABLE,
            )
            .is_some()
    );
    Some(batch)
}

fn read_change(descriptor: i32) -> Change {
    Change::new(descriptor, Filter::Read, Action::Disable, 0)
}

fn assert_statuses(batch: &DisarmBatch, expected: &[CommitStatus]) {
    let actual = batch
        .outcomes()
        .map(|outcome| outcome.commit())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}
