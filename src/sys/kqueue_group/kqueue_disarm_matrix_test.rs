//! Cross-registration kqueue disarm submission and reduction evidence.

use std::io;

use crate::{CommitStatus, Interest, RegistrationId, observe_recovery::DisarmOutcome};

use super::{
    kqueue::{KeventBatch, Kqueue},
    kqueue_change::{Action, Change, Filter},
    kqueue_codec::{classify_apply_error, target_applies_interrupted_changes},
    kqueue_disarm::{DisarmBatch, DisarmChanges, DisarmExecutor, FilterApply, NativeApply},
};

struct MisreportedChanges {
    items: std::vec::IntoIter<Change>,
    advertised: usize,
}

impl Iterator for MisreportedChanges {
    type Item = Change;

    fn next(&mut self) -> Option<Self::Item> {
        self.items.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.advertised, Some(self.advertised))
    }
}

impl ExactSizeIterator for MisreportedChanges {
    fn len(&self) -> usize {
        self.advertised
    }
}

#[cfg(target_os = "macos")]
const _: [(); 0] = [(); target_applies_interrupted_changes() as usize];
#[cfg(any(target_os = "freebsd", target_os = "netbsd"))]
const _: [(); 1] = [(); target_applies_interrupted_changes() as usize];

#[derive(Debug)]
struct RecordingExecutor {
    receipts: [FilterApply; 5],
    submitted: Vec<Change>,
    submissions: usize,
}

impl DisarmExecutor for RecordingExecutor {
    fn apply(&mut self, changes: DisarmChanges<'_>) -> NativeApply {
        self.submissions += 1;
        self.submitted.extend(changes);
        NativeApply::Receipts(self.receipts.len())
    }

    fn receipt(&self, index: usize, returned: usize, _expected: Change) -> FilterApply {
        self.receipts
            .get(index)
            .copied()
            .filter(|_| index < returned)
            .unwrap_or(FilterApply::Unknown(None))
    }
}

#[test]
fn one_batch_preserves_change_outcome_and_source_order() -> io::Result<()> {
    let mut batch =
        DisarmBatch::new(3).ok_or_else(|| io::Error::other("disarm batch storage unavailable"))?;
    push(&mut batch, 1, 11, Interest::READABLE)?;
    push(&mut batch, 2, 12, Interest::READABLE | Interest::WRITABLE)?;
    push(&mut batch, 3, 13, Interest::READABLE | Interest::WRITABLE)?;
    let mut executor = RecordingExecutor {
        receipts: [
            FilterApply::Applied,
            FilterApply::NotApplied(71),
            FilterApply::NotApplied(72),
            FilterApply::Applied,
            FilterApply::Unknown(Some(73)),
        ],
        submitted: Vec::new(),
        submissions: 0,
    };

    let error = batch
        .submit(&mut executor)
        .err()
        .ok_or_else(|| io::Error::other("heterogeneous batch unexpectedly succeeded"))?;

    assert_eq!(executor.submissions, 1);
    assert_eq!(
        executor.submitted,
        [
            change(11, Filter::Read),
            change(12, Filter::Read),
            change(12, Filter::Write),
            change(13, Filter::Read),
            change(13, Filter::Write),
        ]
    );
    assert_eq!(error.raw_os_error(), Some(71));
    assert_eq!(
        batch.outcomes().collect::<Vec<_>>(),
        [
            outcome(1, CommitStatus::Applied),
            outcome(2, CommitStatus::NotApplied),
            outcome(3, CommitStatus::Unknown),
        ]
    );
    Ok(())
}

#[test]
fn target_selected_interruption_policy_is_exact() {
    let result = classify_apply_error(
        io::Error::from(io::ErrorKind::Interrupted),
        target_applies_interrupted_changes(),
    );
    #[cfg(target_os = "macos")]
    assert!(matches!(result, NativeApply::Unknown(_)));
    #[cfg(any(target_os = "freebsd", target_os = "netbsd"))]
    assert!(matches!(result, NativeApply::AppliedWithoutReceipts));
}

#[test]
fn native_staging_rejects_misreported_exact_size_iterators() -> io::Result<()> {
    let queue = Kqueue::new()?;
    let expected = change(5, Filter::Read);
    for (items, advertised) in [(vec![expected], 2), (vec![expected, expected], 1)] {
        let mut native = KeventBatch::new(2, 2)
            .ok_or_else(|| io::Error::other("native receipt storage unavailable"))?;
        let changes = MisreportedChanges {
            items: items.into_iter(),
            advertised,
        };

        assert!(matches!(
            queue.apply_batch(changes, &mut native),
            NativeApply::Unknown(_)
        ));
    }
    Ok(())
}

#[test]
fn test_receipt_staging_rejects_uninitialized_holes() -> io::Result<()> {
    let mut native = KeventBatch::new(2, 2)
        .ok_or_else(|| io::Error::other("native receipt storage unavailable"))?;
    let expected = change(5, Filter::Read);

    assert!(native.stage_receipt(1, expected, 0, true).is_none());
    assert!(native.stage_receipt(0, expected, 0, true).is_some());
    assert!(native.stage_receipt(1, expected, 0, true).is_some());
    assert_eq!(native.receipt(0, 2, expected), FilterApply::Applied);
    Ok(())
}

fn push(
    batch: &mut DisarmBatch,
    registration: u64,
    descriptor: i32,
    interest: Interest,
) -> io::Result<()> {
    batch
        .push(RegistrationId::new(registration), descriptor, interest)
        .ok_or_else(|| io::Error::other("disarm plan exceeded retained storage"))
}

const fn change(descriptor: i32, filter: Filter) -> Change {
    Change::new(descriptor, filter, Action::Disable, 0)
}

const fn outcome(registration: u64, commit: CommitStatus) -> DisarmOutcome {
    DisarmOutcome::new(RegistrationId::new(registration), commit)
}
