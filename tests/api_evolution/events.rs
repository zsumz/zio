//! Event, readiness, key, and wait contracts.

use zio::{Error, Event, Events, Key, Readiness, Wait};

use super::support::*;

#[test]
fn flag_sets_support_standard_set_operators() {
    let _: [zio::Interest; 4] = [
        zio::Interest::EMPTY,
        zio::Interest::READABLE,
        zio::Interest::WRITABLE,
        zio::Interest::ALL,
    ];
    let _: [Readiness; 6] = [
        Readiness::EMPTY,
        Readiness::READABLE,
        Readiness::WRITABLE,
        Readiness::READ_CLOSED,
        Readiness::WRITE_CLOSED,
        Readiness::ERROR,
    ];
    let _ = zio::Interest::is_empty as fn(zio::Interest) -> bool;
    let _ = zio::Interest::contains as fn(zio::Interest, zio::Interest) -> bool;
    let _ = zio::Interest::intersects as fn(zio::Interest, zio::Interest) -> bool;
    let _ = zio::Interest::union as fn(zio::Interest, zio::Interest) -> zio::Interest;
    let _ = zio::Interest::intersection as fn(zio::Interest, zio::Interest) -> zio::Interest;
    let _ = zio::Interest::difference as fn(zio::Interest, zio::Interest) -> zio::Interest;
    let _ =
        zio::Interest::symmetric_difference as fn(zio::Interest, zio::Interest) -> zio::Interest;
    let _ = zio::Interest::complement as fn(zio::Interest) -> zio::Interest;
    let _ = zio::Interest::is_readable as fn(zio::Interest) -> bool;
    let _ = zio::Interest::is_writable as fn(zio::Interest) -> bool;
    let _ = Readiness::is_empty as fn(Readiness) -> bool;
    let _ = Readiness::contains as fn(Readiness, Readiness) -> bool;
    let _ = Readiness::intersects as fn(Readiness, Readiness) -> bool;
    let _ = Readiness::union as fn(Readiness, Readiness) -> Readiness;
    let _ = Readiness::intersection as fn(Readiness, Readiness) -> Readiness;
    let _ = Readiness::difference as fn(Readiness, Readiness) -> Readiness;
    let _ = Readiness::symmetric_difference as fn(Readiness, Readiness) -> Readiness;
    let _ = Readiness::complement as fn(Readiness) -> Readiness;
    let _ = Readiness::is_readable as fn(Readiness) -> bool;
    let _ = Readiness::is_writable as fn(Readiness) -> bool;
    let _ = Readiness::is_read_closed as fn(Readiness) -> bool;
    let _ = Readiness::is_write_closed as fn(Readiness) -> bool;
    let _ = Readiness::is_error as fn(Readiness) -> bool;
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
    let _ = Wait::timeout as fn(Wait) -> Option<core::time::Duration>;
    assert_from::<Wait, core::time::Duration>();
    assert_from::<Wait, Option<core::time::Duration>>();
}

#[test]
fn event_batches_support_standard_iteration() {
    let _ = Events::with_capacity as fn(usize) -> Result<Events, Error>;
    let _ = Events::capacity as fn(&Events) -> usize;
    let _ = Events::len as fn(&Events) -> usize;
    let _ = Events::is_empty as fn(&Events) -> bool;
    assert_slice::<Events, Event>();
    let _ = Events::as_slice as fn(&Events) -> &[Event];
    let _ = Events::get as fn(&Events, usize) -> Option<&Event>;
    let _ = Events::remaining_capacity as fn(&Events) -> usize;
    let _ = Events::is_full as fn(&Events) -> bool;
    let _ = Events::clear as fn(&mut Events);
    let _ = assert_event_iterators as fn(&mut Events);
    let _ = assert_owned_event_iterator as fn(Events);
}

#[test]
fn events_expose_direct_classification_and_readiness() {
    let _ = Event::key as fn(Event) -> Key;
    let _ = Event::registration as fn(Event) -> Option<zio::Registration>;
    let _ = Event::readiness as fn(Event) -> Option<Readiness>;
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
    let _ = Key::new as fn(u64) -> Key;
    let _ = Key::get as fn(Key) -> u64;
    assert_from::<Key, u64>();
    assert_from::<u64, Key>();
    let _: Result<Key, core::num::TryFromIntError> = Key::try_from(0_usize);
    let _: Result<usize, core::num::TryFromIntError> = usize::try_from(Key::ZERO);
    assert_display::<Key>();
}
