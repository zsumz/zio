//! Public poller diagnostics.

use crate::{CapacityKind, CapacityReason, Error, Key, Poll};

const HAS_NATIVE_BACKEND: bool = Poll::has_native_backend();

#[test]
fn native_backend_query_matches_target_selection() {
    assert_eq!(
        HAS_NATIVE_BACKEND,
        cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        ))
    );
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
)))]
#[test]
fn unsupported_target_rejects_construction_before_capacity_validation() {
    assert!(matches!(
        Poll::with_capacity(0, 0),
        Err(Error::UnsupportedPlatform)
    ));
}

#[test]
fn zero_capacities_report_their_kind() {
    assert!(matches!(
        Poll::with_capacity(0, 1),
        Err(Error::Capacity {
            kind: CapacityKind::Event,
            limit: 0,
            reason: CapacityReason::Zero,
        })
    ));
    assert!(matches!(
        Poll::with_capacity(1, 0),
        Err(Error::Capacity {
            kind: CapacityKind::Registration,
            limit: 0,
            reason: CapacityReason::Zero,
        })
    ));
}

#[test]
#[cfg(target_pointer_width = "64")]
fn oversized_registration_capacity_reports_the_backend_limit() {
    assert!(matches!(
        Poll::with_capacity(1, usize::MAX),
        Err(Error::Capacity {
            kind: CapacityKind::Registration,
            limit: usize::MAX,
            reason: CapacityReason::BackendLimit,
        })
    ));
}

#[test]
#[cfg(target_os = "linux")]
fn oversized_event_capacity_reports_the_backend_limit() {
    assert!(matches!(
        Poll::with_capacity(usize::MAX, 1),
        Err(Error::Capacity {
            kind: CapacityKind::Event,
            limit: usize::MAX,
            reason: CapacityReason::BackendLimit,
        })
    ));
}

#[test]
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
fn oversized_kqueue_observation_capacity_reports_the_backend_limit() -> Result<(), Error> {
    let limit = usize::try_from(u32::MAX).map_err(|_| Error::Invariant)?;
    assert!(matches!(
        Poll::with_capacity(1, limit),
        Err(Error::Capacity {
            kind: CapacityKind::Registration,
            limit: actual,
            reason: CapacityReason::BackendLimit,
        }) if actual == limit
    ));
    Ok(())
}

#[test]
fn debug_output_is_backend_neutral() -> Result<(), crate::Error> {
    let mut poll = Poll::with_capacity(3, 5)?;

    assert_eq!(
        format!("{poll:?}"),
        concat!(
            "Poll { event_capacity: 3, registration_capacity: 5, ",
            "registration_count: 0, remaining_registration_capacity: 5, ",
            "wake_key: None, .. }"
        )
    );
    let waker = poll.waker(Key::new(7))?;
    assert_eq!(format!("{waker:?}"), "Waker { key: Key(7), .. }");
    Ok(())
}

#[test]
fn waker_identity_tracks_the_keyed_poller_target() -> Result<(), crate::Error> {
    let key = Key::new(8);
    let mut first_poll = Poll::with_capacity(1, 1)?;
    let mut second_poll = Poll::with_capacity(1, 1)?;
    let first = first_poll.waker(key)?;
    let clone = first.clone();
    let reacquired = first_poll.waker(key)?;
    let distinct = second_poll.waker(key)?;

    assert!(first.will_wake(&clone));
    assert!(first.will_wake(&reacquired));
    assert!(clone.will_wake(&first));
    assert!(!first.will_wake(&distinct));
    Ok(())
}

#[test]
fn waker_can_outlive_its_poller() -> Result<(), crate::Error> {
    let mut poll = Poll::with_capacity(1, 1)?;
    let waker = poll.waker(Key::new(9))?;

    drop(poll);

    waker.wake()
}

#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
#[test]
fn undersized_destination_does_not_consume_deferred_wake() -> Result<(), Error> {
    let mut poll = Poll::with_capacity(2, 1)?;
    let key = Key::new(10);
    let _waker = poll.waker(key)?;
    poll.deferred_wake = true;
    let mut undersized = crate::Events::with_capacity(1)?;

    assert!(matches!(
        poll.wait(&mut undersized, crate::Wait::NoBlock),
        Err(Error::EventsTooSmall {
            required: 2,
            actual: 1,
        })
    ));
    assert!(undersized.is_empty());
    assert!(poll.deferred_wake);

    let mut events = poll.events()?;
    assert!(poll.wait(&mut events, crate::Wait::NoBlock)?.is_complete());
    assert_eq!(events.as_slice(), &[crate::Event::Wake { key }]);
    assert!(!poll.deferred_wake);
    Ok(())
}
