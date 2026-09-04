//! Caller-arena phase transition regressions.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::{Error, Event, Events, Key, Readiness, Registration};

use super::{backend::epoll_test_flags, epoll::EpollBatch};

#[test]
fn unstamped_storage_never_exposes_an_event() -> Result<(), Box<dyn std::error::Error>> {
    let mut batch =
        EpollBatch::new(1).ok_or_else(|| std::io::Error::other("batch construction failed"))?;
    let mut events = Events::with_capacity(1)?;

    assert!(matches!(
        batch.translate(&mut events, 1, None, |_| Ok(Some(classified_zero()))),
        Err(Error::Invariant)
    ));
    assert!(events.is_empty());
    Ok(())
}

#[test]
fn dense_tail_staging_preserves_every_resource() -> Result<(), Box<dyn std::error::Error>> {
    const CAPACITY: usize = 1_024;
    let flags = epoll_test_flags()[0];
    let raw: Vec<_> = (1..=u64::try_from(CAPACITY)?)
        .map(|token| (flags, token))
        .collect();
    let mut batch =
        EpollBatch::new(CAPACITY).ok_or_else(|| std::io::Error::other("batch failed"))?;
    let mut events = Events::with_capacity(CAPACITY + 1)?;
    let observed = batch
        .stage(&mut events, &raw)
        .ok_or_else(|| std::io::Error::other("staging failed"))?;

    batch.translate(&mut events, observed, None, |token| {
        Ok(Some(classified(token, Key::new(token + 10_000))))
    })?;

    assert_eq!(events.len(), CAPACITY);
    for (index, event) in events.iter().enumerate() {
        assert_eq!(
            *event,
            Event::Resource {
                registration: registration(u64::try_from(index)? + 1),
                key: Key::new(u64::try_from(index)? + 10_001),
                readiness: Readiness::READABLE,
            }
        );
    }
    Ok(())
}

#[test]
fn sparse_tail_translation_preserves_live_order_and_wake_last()
-> Result<(), Box<dyn std::error::Error>> {
    let flags = epoll_test_flags()[0];
    let raw = [(flags, 11), (flags, 0), (flags, 12), (flags, 13)];
    let mut batch =
        EpollBatch::new(raw.len()).ok_or_else(|| std::io::Error::other("batch failed"))?;
    let mut events = Events::with_capacity(raw.len())?;
    let observed = batch
        .stage(&mut events, &raw)
        .ok_or_else(|| std::io::Error::other("staging failed"))?;

    batch.translate(&mut events, observed, Some(Key::new(99)), |token| {
        Ok((token == 13).then_some(classified(token, Key::new(93))))
    })?;

    assert_eq!(
        events.as_slice(),
        &[
            Event::Resource {
                registration: registration(13),
                key: Key::new(93),
                readiness: Readiness::READABLE,
            },
            Event::Wake { key: Key::new(99) },
        ]
    );
    Ok(())
}

#[test]
fn mismatched_stamp_is_rejected_and_invalidated() -> Result<(), Box<dyn std::error::Error>> {
    let flags = epoll_test_flags()[0];
    let mut batch = EpollBatch::new(2).ok_or_else(|| std::io::Error::other("batch failed"))?;
    let mut staged = Events::with_capacity(2)?;
    let mut replacement = Events::with_capacity(2)?;
    let observed = batch
        .stage(&mut staged, &[(flags, 1)])
        .ok_or_else(|| std::io::Error::other("staging failed"))?;

    assert!(matches!(
        batch.translate(&mut replacement, observed, None, |_| {
            Ok(Some(classified_zero()))
        }),
        Err(Error::Invariant)
    ));
    assert!(staged.is_empty());
    assert!(replacement.is_empty());
    assert!(matches!(
        batch.translate(&mut staged, observed, None, |_| Ok(Some(classified_zero()))),
        Err(Error::Invariant)
    ));

    let observed = batch
        .stage(&mut staged, &[(flags, 2)])
        .ok_or_else(|| std::io::Error::other("restaging failed"))?;
    assert!(matches!(
        batch.translate(&mut staged, observed + 1, None, |_| {
            Ok(Some(classified_zero()))
        }),
        Err(Error::Invariant)
    ));
    assert!(staged.is_empty());
    Ok(())
}

#[test]
fn failed_classification_leaves_storage_empty_and_reusable()
-> Result<(), Box<dyn std::error::Error>> {
    let flags = epoll_test_flags()[0];
    let mut batch = EpollBatch::new(2).ok_or_else(|| std::io::Error::other("batch failed"))?;
    let mut events = Events::with_capacity(2)?;
    let first = [(flags, 1), (flags, 2)];
    let observed = batch
        .stage(&mut events, &first)
        .ok_or_else(|| std::io::Error::other("first staging failed"))?;

    let result = batch.translate(&mut events, observed, None, |token| {
        if token == 2 {
            Err(Error::Invariant)
        } else {
            Ok(Some(classified(token, Key::new(token))))
        }
    });
    assert!(matches!(result, Err(Error::Invariant)));
    assert!(events.is_empty());

    let second = [(flags, 3)];
    let observed = batch
        .stage(&mut events, &second)
        .ok_or_else(|| std::io::Error::other("second staging failed"))?;
    batch.translate(&mut events, observed, None, |token| {
        Ok(Some(classified(token, Key::new(token))))
    })?;
    assert_eq!(
        events.as_slice(),
        &[Event::Resource {
            registration: registration(3),
            key: Key::new(3),
            readiness: Readiness::READABLE,
        }]
    );
    Ok(())
}

#[test]
fn panicking_classification_invalidates_storage_before_unwind()
-> Result<(), Box<dyn std::error::Error>> {
    let flags = epoll_test_flags()[0];
    let mut batch = EpollBatch::new(2).ok_or_else(|| std::io::Error::other("batch failed"))?;
    let mut events = Events::with_capacity(2)?;
    let observed = batch
        .stage(&mut events, &[(flags, 1), (flags, 2)])
        .ok_or_else(|| std::io::Error::other("staging failed"))?;

    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let _ = batch.translate(&mut events, observed, None, |token| {
            assert_ne!(token, 2);
            Ok(Some(classified(token, Key::new(token))))
        });
    }));
    assert!(unwind.is_err());
    assert!(events.is_empty());
    assert!(matches!(
        batch.translate(&mut events, observed, None, |_| Ok(Some(classified_zero()))),
        Err(Error::Invariant)
    ));

    let observed = batch
        .stage(&mut events, &[(flags, 3)])
        .ok_or_else(|| std::io::Error::other("restaging failed"))?;
    batch.translate(&mut events, observed, None, |token| {
        Ok(Some(classified(token, Key::new(token))))
    })?;
    assert_eq!(
        events.get(0),
        Some(&Event::Resource {
            registration: registration(3),
            key: Key::new(3),
            readiness: Readiness::READABLE,
        })
    );
    Ok(())
}

const fn classified(token: u64, key: Key) -> (Registration, Key) {
    (registration(token), key)
}

const fn classified_zero() -> (Registration, Key) {
    classified(1, Key::ZERO)
}

const fn registration(token: u64) -> Registration {
    Registration::test(token)
}
