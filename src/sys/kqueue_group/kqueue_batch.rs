//! Safe access to kernel-written kqueue batch prefixes.

use std::os::fd::RawFd;

use super::{
    kqueue::{KeventBatch, initialized_event},
    kqueue_arena::KqueueArena,
    kqueue_change::{Change, RawKevent},
    kqueue_codec::{decode_filter, decode_receipt, has_eof, has_native_error, is_missing_disable},
    kqueue_disarm::FilterApply,
};

impl KeventBatch {
    pub(super) fn new(event_capacity: usize, change_capacity: usize) -> Option<Self> {
        KqueueArena::new(event_capacity, change_capacity).map(|arena| Self {
            arena,
            observed: 0,
            receipts: 0,
        })
    }

    pub(super) fn event(&self, index: usize, observed: usize) -> Option<RawKevent> {
        if observed != self.observed || index >= observed {
            return None;
        }
        let event = initialized_event(self.arena.event_slot(index)?);
        Some(RawKevent::new(
            RawFd::try_from(event.ident).ok()?,
            decode_filter(i64::from(event.filter)),
            event.udata as usize as u64,
            has_eof(u64::from(event.flags)),
            has_native_error(u64::from(event.flags)),
            event.fflags,
        ))
    }

    pub(super) fn receipt(&self, index: usize, returned: usize, expected: Change) -> FilterApply {
        let event = (returned == self.receipts && index < returned)
            .then(|| self.arena.receipt_slot(index))
            .flatten()
            .map(initialized_event);
        match decode_receipt(event, expected) {
            FilterApply::NotApplied(code) if is_missing_disable(code, expected) => {
                FilterApply::AlreadyAbsent
            }
            outcome => outcome,
        }
    }
}
