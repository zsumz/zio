//! Safe encoding and validation for native kqueue values.

use std::{io, os::fd::RawFd};

use super::{
    kqueue::{KeventBatch, Kqueue, empty_kevent},
    kqueue_arena::{StageError, StagedChanges},
    kqueue_change::{Action, Change, ChangeList, Filter, Receipt, Receipts},
    kqueue_disarm::{FilterApply, NativeApply},
};

/// Initialized native registration input that never requests a receipt.
#[repr(transparent)]
pub(super) struct NativeChange(libc::kevent);

#[cfg(test)]
impl NativeChange {
    pub(super) fn requests_receipt(&self) -> bool {
        has_flag(u64::from(self.0.flags), u64::from(libc::EV_RECEIPT))
    }
}

impl KeventBatch {
    pub(super) fn stage_changes<I>(
        &mut self,
        changes: I,
    ) -> Result<StagedChanges<'_, libc::kevent>, NativeApply>
    where
        I: ExactSizeIterator<Item = Change>,
    {
        self.arena
            .stage(
                changes,
                encode_change,
                &mut self.observed,
                &mut self.receipts,
            )
            .map_err(|error| match error {
                StageError::Encode(source) => NativeApply::Unknown(source),
                StageError::Bounds | StageError::Misreported => {
                    NativeApply::Unknown(protocol_error())
                }
            })
    }

    #[cfg(test)]
    fn stage_test_receipt(&mut self, index: usize, event: libc::kevent) -> Option<()> {
        if index > self.receipts {
            return None;
        }
        self.arena.receipt_slot_mut(index)?.write(event);
        self.receipts = self.receipts.max(index.checked_add(1)?);
        Some(())
    }
}

impl Kqueue {
    pub(super) fn apply(&self, changes: &ChangeList) -> io::Result<Receipts> {
        let changes = changes.as_slice();
        let mut input = [empty_kevent(), empty_kevent()];
        let mut output = [empty_kevent(), empty_kevent()];
        for (destination, change) in input.iter_mut().zip(changes) {
            *destination = encode_change(*change)?;
        }
        let mut receipts = Receipts::new(changes.len());
        match self.submit_native(&input[..changes.len()], &mut output[..changes.len()]) {
            NativeApply::AppliedWithoutReceipts => {
                for (index, change) in changes.iter().copied().enumerate() {
                    receipts.set(index, Receipt::new(change.action(), None))?;
                }
            }
            NativeApply::Receipts(returned) if returned == changes.len() => {
                for (index, change) in changes.iter().copied().enumerate() {
                    let error = match decode_receipt(output.get(index), change) {
                        FilterApply::Applied => None,
                        FilterApply::NotApplied(code) => Some(code),
                        FilterApply::AlreadyAbsent | FilterApply::Unknown(_) => {
                            return Err(protocol_error());
                        }
                    };
                    receipts.set(index, Receipt::new(change.action(), error))?;
                }
            }
            NativeApply::Receipts(_) => return Err(protocol_error()),
            NativeApply::Unknown(source) => return Err(source),
        }
        Ok(receipts)
    }

    pub(super) fn apply_batch<I>(&self, changes: I, batch: &mut KeventBatch) -> NativeApply
    where
        I: ExactSizeIterator<Item = Change>,
    {
        let staged = match batch.stage_changes(changes) {
            Ok(staged) => staged,
            Err(failure) => return failure,
        };
        self.submit_staged_native(staged)
    }
}

#[cfg(test)]
impl KeventBatch {
    #[allow(
        clippy::unnecessary_fallible_conversions,
        reason = "macOS kevent data is isize while BSD targets use i64"
    )]
    pub(super) fn stage_receipt(
        &mut self,
        index: usize,
        observed: Change,
        error: i32,
        marked: bool,
    ) -> Option<()> {
        let mut event = encode_change(observed).ok()?;
        event.flags = if marked { libc::EV_ERROR } else { 0 };
        event.data = error.try_into().ok()?;
        self.stage_test_receipt(index, event)
    }
}

pub(super) const fn target_applies_interrupted_changes() -> bool {
    #[cfg(any(target_os = "freebsd", target_os = "netbsd"))]
    {
        true
    }
    #[cfg(target_os = "macos")]
    {
        false
    }
}

#[cfg(test)]
pub(super) const fn missing_entry_error_code() -> i32 {
    libc::ENOENT
}

pub(super) fn classify_apply_error(
    error: io::Error,
    interrupted_changes_apply: bool,
) -> NativeApply {
    if error.kind() == io::ErrorKind::Interrupted && interrupted_changes_apply {
        NativeApply::AppliedWithoutReceipts
    } else {
        NativeApply::Unknown(error)
    }
}

pub(super) fn encode_change(change: Change) -> io::Result<libc::kevent> {
    encode_change_with_receipt(change, true)
}

pub(super) fn encode_registration_change(change: Change) -> io::Result<NativeChange> {
    encode_change_with_receipt(change, false).map(NativeChange)
}

fn encode_change_with_receipt(change: Change, include_receipt: bool) -> io::Result<libc::kevent> {
    let mut event = empty_kevent();
    event.ident = libc::uintptr_t::try_from(change.ident())
        .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
    event.filter = match change.filter() {
        Filter::Read => libc::EVFILT_READ,
        Filter::Write => libc::EVFILT_WRITE,
        Filter::User => libc::EVFILT_USER,
        Filter::Unknown => 0,
    };
    let receipt = if include_receipt { libc::EV_RECEIPT } else { 0 };
    event.flags = match change.action() {
        Action::AddEnabled => libc::EV_ADD | libc::EV_ENABLE | receipt,
        Action::AddDisabled => libc::EV_ADD | libc::EV_DISABLE | receipt,
        Action::AddUser => libc::EV_ADD | libc::EV_CLEAR | receipt,
        Action::Delete => libc::EV_DELETE | receipt,
        Action::Disable => libc::EV_DISABLE | receipt,
        Action::Trigger => libc::EV_ADD | receipt,
    };
    event.fflags = u32::from(change.action() == Action::Trigger) * libc::NOTE_TRIGGER;
    event.udata = usize::try_from(change.token())
        .map_err(|_| io::Error::from_raw_os_error(libc::EOVERFLOW))?
        as *mut libc::c_void;
    Ok(event)
}

pub(super) fn decode_filter(filter: i64) -> Filter {
    if filter == i64::from(libc::EVFILT_READ) {
        Filter::Read
    } else if filter == i64::from(libc::EVFILT_WRITE) {
        Filter::Write
    } else if filter == i64::from(libc::EVFILT_USER) {
        Filter::User
    } else {
        Filter::Unknown
    }
}

pub(super) fn ident(event: &libc::kevent) -> Option<RawFd> {
    RawFd::try_from(event.ident).ok()
}

pub(super) fn has_flag(flags: u64, expected: u64) -> bool {
    flags & expected != 0
}

pub(super) fn has_eof(flags: u64) -> bool {
    has_flag(flags, u64::from(libc::EV_EOF))
}

pub(super) fn has_native_error(flags: u64) -> bool {
    has_flag(flags, u64::from(libc::EV_ERROR))
}

pub(super) fn is_missing_disable(code: i32, expected: Change) -> bool {
    code == libc::ENOENT && expected.action() == Action::Disable
}

pub(super) fn decode_receipt(event: Option<&libc::kevent>, expected: Change) -> FilterApply {
    let Some(event) = event else {
        return FilterApply::Unknown(None);
    };
    if ident(event) != Some(expected.ident())
        || decode_filter(i64::from(event.filter)) != expected.filter()
        || !has_flag(u64::from(event.flags), u64::from(libc::EV_ERROR))
    {
        return FilterApply::Unknown(None);
    }
    match i32::try_from(event.data) {
        Ok(0) => FilterApply::Applied,
        Ok(code) if code > 0 => FilterApply::NotApplied(code),
        _ => FilterApply::Unknown(None),
    }
}

fn protocol_error() -> io::Error {
    io::Error::from_raw_os_error(libc::EIO)
}
