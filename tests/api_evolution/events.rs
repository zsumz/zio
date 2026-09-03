//! Event, readiness, key, and wait contracts.

use zio::{Event, Events, Key, Readiness, Wait};

use super::support::*;

#[test]
fn flag_sets_support_standard_set_operators() {
    let _ =
        zio::Interest::symmetric_difference as fn(zio::Interest, zio::Interest) -> zio::Interest;
    let _ = Readiness::symmetric_difference as fn(Readiness, Readiness) -> Readiness;
    let _ = zio::Interest::complement as fn(zio::Interest) -> zio::Interest;
    let _ = Readiness::complement as fn(Readiness) -> Readiness;
    assert_eq!(
        zio::Interest::ALL,
        zio::Interest::READABLE | zio::Interest::WRITABLE
    );
    assert!(Readiness::ALL.contains(Readiness::ERROR));
    assert_set_operators::<zio::Interest>();
    assert_set_operators::<Readiness>();
}

#[test]
fn event_and_wait_values_are_hashable() {
    assert_hash::<Event>();
    assert_hash::<Wait>();
    assert_from::<Wait, core::time::Duration>();
    assert_from::<Wait, Option<core::time::Duration>>();
}

#[test]
fn event_batches_support_standard_iteration() {
    assert_slice::<Events, Event>();
    let _ = Events::remaining_capacity as fn(&Events) -> usize;
    let _ = Events::is_full as fn(&Events) -> bool;
    let _ = assert_event_iterators as fn(&mut Events);
    let _ = assert_owned_event_iterator as fn(Events);
}

#[test]
fn events_expose_direct_classification_and_readiness() {
    let _ = Event::is_resource as fn(Event) -> bool;
    let _ = Event::is_wake as fn(Event) -> bool;
    let _ = Event::is_readable as fn(Event) -> bool;
    let _ = Event::is_writable as fn(Event) -> bool;
    let _ = Event::is_read_closed as fn(Event) -> bool;
    let _ = Event::is_write_closed as fn(Event) -> bool;
    let _ = Event::is_error as fn(Event) -> bool;
}

#[test]
fn keys_support_lossless_standard_conversions() {
    assert_from::<Key, u64>();
    assert_from::<u64, Key>();
    let _: Result<Key, core::num::TryFromIntError> = Key::try_from(0_usize);
    let _: Result<usize, core::num::TryFromIntError> = usize::try_from(Key::ZERO);
    assert_display::<Key>();
}
