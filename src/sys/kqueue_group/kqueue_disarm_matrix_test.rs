//! Cross-registration kqueue disarm submission and reduction evidence.

use std::io;

use crate::{CommitStatus, Interest, RecoveryOutcome, RegistrationId};

use super::{
    kqueue_change::{Action, Change, Filter},
    kqueue_codec::{classify_apply_error, target_applies_interrupted_changes},
    kqueue_disarm::{DisarmBatch, DisarmExecutor, FilterApply, NativeApply},
};

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
    fn apply(&mut self, changes: &[Change]) -> NativeApply {
        self.submissions += 1;
        self.submitted.extend_from_slice(changes);
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
        batch.outcomes(),
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

const fn outcome(registration: u64, commit: CommitStatus) -> RecoveryOutcome {
    RecoveryOutcome::new(RegistrationId::new(registration), commit)
}
