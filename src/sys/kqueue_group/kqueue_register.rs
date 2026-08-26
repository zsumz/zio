//! Receipt-free native registration staging over exact rollback policy.

use std::{io, os::fd::RawFd};

use crate::Interest;

use super::{
    kqueue::Kqueue,
    kqueue_change::{Action, Change, Filter},
    kqueue_codec::{NativeChange, encode_registration_change},
    kqueue_disarm::NativeApply,
};

impl Kqueue {
    pub(super) fn apply_registration_native(
        &self,
        descriptor: RawFd,
        token: u64,
        interest: Interest,
    ) -> io::Result<()> {
        debug_assert_ne!(token, 0);
        debug_assert!(!interest.is_empty());
        let native = if interest.is_readable() {
            if interest.is_writable() {
                let changes = [
                    registration_change(descriptor, token, Filter::Read)?,
                    registration_change(descriptor, token, Filter::Write)?,
                ];
                self.submit_changes_native(&changes)
            } else {
                let change = registration_change(descriptor, token, Filter::Read)?;
                self.submit_changes_native(core::slice::from_ref(&change))
            }
        } else {
            let change = registration_change(descriptor, token, Filter::Write)?;
            self.submit_changes_native(core::slice::from_ref(&change))
        };
        match native {
            // A successful change-only submission has zero output entries.
            NativeApply::AppliedWithoutReceipts | NativeApply::Receipts(0) => Ok(()),
            NativeApply::Receipts(_) => Err(io::Error::other(
                "kqueue returned output for a change-only submission",
            )),
            NativeApply::Unknown(source) => Err(source),
        }
    }
}

fn registration_change(descriptor: RawFd, token: u64, filter: Filter) -> io::Result<NativeChange> {
    encode_registration_change(Change::new(descriptor, filter, Action::AddEnabled, token))
}
