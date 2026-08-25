//! Kqueue event normalization truth tables.

use crate::Readiness;

use super::{
    backend::from_kqueue_event,
    kqueue_change::{Filter, RawKevent},
};

#[test]
fn kqueue_readiness_truth_table_is_exhaustive() {
    let filters = [Filter::Read, Filter::Write, Filter::User, Filter::Unknown];
    for filter in filters {
        for eof in [false, true] {
            for native_error in [false, true] {
                for fflags in [0, 1] {
                    let event = RawKevent::new(7, filter, 11, eof, native_error, fflags);
                    assert_eq!(
                        from_kqueue_event(event),
                        expected_readiness(filter, eof, native_error, fflags),
                        "filter {filter:?}, eof {eof}, error {native_error}, fflags {fflags}",
                    );
                }
            }
        }
    }
}

#[test]
fn kqueue_fflags_require_eof_to_become_an_error_hint() {
    let without_eof = RawKevent::new(7, Filter::Read, 11, false, false, 5);
    let with_eof = RawKevent::new(7, Filter::Read, 11, true, false, 5);

    assert_eq!(from_kqueue_event(without_eof), Readiness::READABLE);
    assert_eq!(
        from_kqueue_event(with_eof),
        Readiness::READABLE
            .union(Readiness::READ_CLOSED)
            .union(Readiness::ERROR),
    );
}

#[test]
fn kqueue_pending_io_and_eof_hints_remain_combined() {
    let cases = [
        (
            Filter::Read,
            Readiness::READABLE.union(Readiness::READ_CLOSED),
        ),
        (
            Filter::Write,
            Readiness::WRITABLE.union(Readiness::WRITE_CLOSED),
        ),
    ];

    for (filter, expected) in cases {
        let event = RawKevent::new(7, filter, 11, true, false, 0);
        assert_eq!(from_kqueue_event(event), expected, "filter {filter:?}");
    }
}

fn expected_readiness(filter: Filter, eof: bool, native_error: bool, fflags: u32) -> Readiness {
    let mut readiness = match filter {
        Filter::Read => Readiness::READABLE,
        Filter::Write => Readiness::WRITABLE,
        Filter::User | Filter::Unknown => Readiness::ERROR,
    };
    if eof {
        readiness = readiness.union(match filter {
            Filter::Read => Readiness::READ_CLOSED,
            Filter::Write => Readiness::WRITE_CLOSED,
            Filter::User | Filter::Unknown => Readiness::ERROR,
        });
    }
    if native_error || (eof && fflags != 0) {
        readiness = readiness.union(Readiness::ERROR);
    }
    readiness
}
