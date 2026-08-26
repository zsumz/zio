//! Recovery-state bridge evidence for scoped kqueue deletion.

use std::{cell::RefCell, fs::File, io, num::NonZeroUsize, os::fd::AsRawFd};

use crate::{
    ArmState, CommitStatus, Interest, Key, Mode, RegistrationState, descriptor::Descriptor,
    sys::MutationFailure, table::RegistrationTable,
};

use super::{
    kqueue_change::{Change, ChangeList, Filter, Receipt, Receipts},
    kqueue_policy::{ChangeExecutor, delete_descriptor},
};

#[derive(Default)]
struct Recorder {
    changes: RefCell<Vec<Change>>,
}

impl ChangeExecutor for Recorder {
    fn apply(&self, changes: &ChangeList) -> io::Result<Receipts> {
        self.changes.borrow_mut().extend(changes.as_slice());
        let mut receipts = Receipts::new(changes.as_slice().len());
        for (index, change) in changes.as_slice().iter().enumerate() {
            receipts.set(index, Receipt::new(change.action(), None))?;
        }
        Ok(receipts)
    }
}

#[test]
fn recovery_outcomes_drive_exact_delete_scope() -> Result<(), Box<dyn std::error::Error>> {
    let limit = NonZeroUsize::new(3).ok_or_else(|| io::Error::other("zero table limit"))?;
    let mut table = RegistrationTable::new(limit)?;
    verify(
        &mut table,
        1,
        Interest::READABLE,
        CommitStatus::Applied,
        RegistrationState::Registered {
            arm: ArmState::Disarmed,
        },
        &[Filter::Read],
    )?;
    verify(
        &mut table,
        2,
        Interest::WRITABLE,
        CommitStatus::NotApplied,
        RegistrationState::Registered {
            arm: ArmState::Armed,
        },
        &[Filter::Write],
    )?;
    verify(
        &mut table,
        3,
        Interest::READABLE,
        CommitStatus::Unknown,
        RegistrationState::Uncertain,
        &[Filter::Read, Filter::Write],
    )
}

fn verify(
    table: &mut RegistrationTable,
    key: u64,
    interest: Interest,
    commit: CommitStatus,
    expected_state: RegistrationState,
    expected_filters: &[Filter],
) -> Result<(), Box<dyn std::error::Error>> {
    let descriptor = Descriptor::owned(File::open("/dev/null")?.into());
    let registration =
        table.reserve_descriptor(descriptor, Key::new(key), interest, Mode::OneShot)?;
    assert_eq!(table.apply_disarm(registration, commit)?, expected_state);

    let binding = table.binding(registration, true)?;
    let recorder = Recorder::default();
    delete_descriptor(
        &recorder,
        binding.descriptor.as_raw_fd(),
        binding.interest,
        binding.state,
    )
    .map_err(MutationFailure::into_source)?;
    assert_eq!(
        recorder
            .changes
            .borrow()
            .iter()
            .map(|change| change.filter())
            .collect::<Vec<_>>(),
        expected_filters
    );
    Ok(())
}
