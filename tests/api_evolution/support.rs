//! Trait assertions and event-iterator probes.

use core::{
    iter::FusedIterator,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Sub, SubAssign},
};

use zio::{Error, Events, RecoveryFailure, RecoveryOutcome};

pub(super) fn assert_slice<T: AsRef<[U]>, U>() {}

pub(super) fn assert_ordered<T: Ord>() {}

pub(super) fn assert_hash<T: core::hash::Hash>() {}

pub(super) fn assert_display<T: core::fmt::Display>() {}

pub(super) fn assert_error_ref<T: AsRef<Error>>() {}

pub(super) fn assert_send<T: Send>() {}

pub(super) fn assert_send_sync<T: Send + Sync>() {}

pub(super) fn assert_from<T: From<U>, U>() {}

pub(super) fn assert_event_iterators(events: &mut Events) {
    assert_iterator(events.iter());
    assert_iterator(events.drain());
}

pub(super) fn assert_owned_event_iterator(events: Events) {
    assert_iterator(events.into_iter());
}

pub(super) fn assert_recovery_iterator(failure: &RecoveryFailure) {
    assert_iterator(failure.iter());
    assert_iterator(failure.into_iter());
    let _: Option<&RecoveryOutcome> = failure.into_iter().next();
}

fn assert_iterator<I: DoubleEndedIterator + ExactSizeIterator + FusedIterator>(_: I) {}

pub(super) fn assert_set_operators<T>()
where
    T: BitOr<Output = T>
        + BitOrAssign
        + BitAnd<Output = T>
        + BitAndAssign
        + BitXor<Output = T>
        + BitXorAssign
        + Not<Output = T>
        + Sub<Output = T>
        + SubAssign,
{
}
