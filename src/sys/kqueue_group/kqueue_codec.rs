//! Safe encoding and validation for native kqueue values.

use std::{io, os::fd::RawFd};

use super::{
    kqueue::empty_kevent,
    kqueue_change::{Action, Change, Filter},
    kqueue_disarm::{FilterApply, NativeApply},
};

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

/// Retained native input and receipt storage for bulk changes.
pub(super) struct KeventChangeBatch {
    pub(super) input: Box<[libc::kevent]>,
    pub(super) output: Box<[libc::kevent]>,
}

impl KeventChangeBatch {
    pub(super) fn new(capacity: usize) -> Option<Self> {
        #[cfg(not(target_os = "netbsd"))]
        i32::try_from(capacity).ok()?;
        Some(Self {
            input: native_storage(capacity)?,
            output: native_storage(capacity)?,
        })
    }

    pub(super) fn receipt(&self, index: usize, returned: usize, expected: Change) -> FilterApply {
        match decode_receipt(
            self.output.get(index).filter(|_| index < returned),
            expected,
        ) {
            FilterApply::NotApplied(code)
                if code == libc::ENOENT && expected.action() == Action::Disable =>
            {
                FilterApply::AlreadyAbsent
            }
            outcome => outcome,
        }
    }

    #[cfg(test)]
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
        let event = self.output.get_mut(index)?;
        *event = encode_change(observed).ok()?;
        event.flags = if marked { libc::EV_ERROR } else { 0 };
        event.data = error.try_into().ok()?;
        Some(())
    }
}

impl std::fmt::Debug for KeventChangeBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KeventChangeBatch")
            .field("capacity", &self.input.len())
            .finish_non_exhaustive()
    }
}

pub(super) fn encode_change(change: Change) -> io::Result<libc::kevent> {
    let mut event = empty_kevent();
    event.ident = libc::uintptr_t::try_from(change.ident())
        .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
    event.filter = match change.filter() {
        Filter::Read => libc::EVFILT_READ,
        Filter::Write => libc::EVFILT_WRITE,
        Filter::User => libc::EVFILT_USER,
        Filter::Unknown => 0,
    };
    let receipt = libc::EV_RECEIPT;
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

fn native_storage(capacity: usize) -> Option<Box<[libc::kevent]>> {
    let mut events = Vec::new();
    events.try_reserve_exact(capacity).ok()?;
    events.resize_with(capacity, empty_kevent);
    Some(events.into_boxed_slice())
}
