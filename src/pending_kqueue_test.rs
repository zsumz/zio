//! Sparse kqueue observation coalescing and storage-reuse proofs.

use core::num::{NonZeroU32, NonZeroUsize};

use crate::{Error, Key, Readiness, pending_kqueue::KqueuePending, token::encode};

#[test]
fn sparse_clear_reuses_storage_and_resets_generation_guard() -> Result<(), Error> {
    let registrations = NonZeroUsize::new(8).ok_or(Error::Invariant)?;
    let mut pending = KqueuePending::new(registrations)?;
    let first = registration(6, 1)?;
    let next = registration(6, 2)?;

    pending.add(first, Key::new(7), Readiness::READABLE)?;
    assert!(pending.add(next, Key::new(8), Readiness::WRITABLE).is_err());
    pending.clear();
    pending.add(next, Key::new(8), Readiness::WRITABLE)?;

    assert_eq!(pending.as_slice().len(), 1);
    assert_eq!(pending.as_slice()[0].registration, next);
    Ok(())
}

#[cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]
#[test]
fn add_coalesce_and_clear_are_allocation_free() -> Result<(), Error> {
    let capacity = NonZeroUsize::new(2).ok_or(Error::Invariant)?;
    let mut pending = KqueuePending::new(capacity)?;
    let first = registration(0, 1)?;
    let second = registration(1, 1)?;
    let mut result = Ok(());

    let allocations = allocation_counter::measure(|| {
        result = pending.add(first, Key::new(9), Readiness::READABLE);
        if result.is_ok() {
            result = pending.add(second, Key::new(10), Readiness::READABLE);
        }
        if result.is_ok() {
            result = pending.add(first, Key::new(9), Readiness::WRITABLE);
        }
        let _ = pending.delivery_selection(1);
        pending.clear();
    });

    result?;
    assert_eq!(allocations.count_total, 0);
    assert_eq!(allocations.bytes_total, 0);
    assert!(pending.as_slice().is_empty());
    Ok(())
}

#[test]
fn delivery_rotates_with_full_batches_without_reordering_output() -> Result<(), Error> {
    let capacity = NonZeroUsize::new(3).ok_or(Error::Invariant)?;
    let mut pending = KqueuePending::new(capacity)?;
    let registrations = [
        registration(0, 1)?,
        registration(1, 1)?,
        registration(2, 1)?,
    ];

    add_all(&mut pending, &registrations)?;
    assert_delivery(&mut pending, 2, &registrations[0..2])?;
    pending.clear();

    add_all(&mut pending, &registrations)?;
    assert_delivery(&mut pending, 2, &[registrations[0], registrations[2]])?;
    pending.clear();

    add_all(&mut pending, &registrations)?;
    assert_delivery(&mut pending, 2, &registrations[1..3])?;
    pending.clear();

    add_all(&mut pending, &registrations[1..])?;
    assert_delivery(&mut pending, 2, &registrations[1..])?;
    Ok(())
}

#[test]
fn capacity_plus_one_and_two_keep_every_batch_full() -> Result<(), Error> {
    for (registration_count, event_capacity) in [(5, 4), (6, 4)] {
        let capacity = NonZeroUsize::new(registration_count).ok_or(Error::Invariant)?;
        let mut pending = KqueuePending::new(capacity)?;
        let registrations = (0..registration_count)
            .map(|slot| u32::try_from(slot).map_err(|_| Error::Invariant))
            .map(|slot| slot.and_then(|slot| registration(slot, 1)))
            .collect::<Result<Vec<_>, _>>()?;

        for _ in 0..registration_count.saturating_mul(2) {
            add_all(&mut pending, &registrations)?;
            let selection = pending.delivery_selection(event_capacity);
            assert_eq!(selection.len(), event_capacity);
            assert_eq!(
                selection.try_iter(pending.as_slice())?.count(),
                event_capacity
            );
            pending.clear();
        }
    }
    Ok(())
}

#[test]
fn capacity_one_cycles_across_the_ready_set() -> Result<(), Error> {
    let capacity = NonZeroUsize::new(3).ok_or(Error::Invariant)?;
    let mut pending = KqueuePending::new(capacity)?;
    let registrations = [
        registration(0, 1)?,
        registration(1, 1)?,
        registration(2, 1)?,
    ];

    for expected in registrations.into_iter().cycle().take(6) {
        add_all(&mut pending, &registrations)?;
        assert_delivery(&mut pending, 1, &[expected])?;
        pending.clear();
    }
    Ok(())
}

#[test]
fn a_disappeared_cursor_restarts_at_first_observation() -> Result<(), Error> {
    let capacity = NonZeroUsize::new(3).ok_or(Error::Invariant)?;
    let mut pending = KqueuePending::new(capacity)?;
    let registrations = [
        registration(0, 1)?,
        registration(1, 1)?,
        registration(2, 1)?,
    ];

    add_all(&mut pending, &registrations)?;
    assert_delivery(&mut pending, 2, &registrations[0..2])?;
    pending.clear();
    pending.add(registrations[0], Key::ZERO, Readiness::READABLE)?;
    pending.add(registrations[2], Key::ZERO, Readiness::READABLE)?;
    assert_delivery(&mut pending, 1, &[registrations[0]])?;
    Ok(())
}

#[test]
fn cyclic_selection_properties_hold_across_small_capacity_pairs() -> Result<(), Error> {
    for registration_count in 1..=24 {
        let capacity = NonZeroUsize::new(registration_count).ok_or(Error::Invariant)?;
        let registrations = (0..registration_count)
            .map(|slot| u32::try_from(slot).map_err(|_| Error::Invariant))
            .map(|slot| slot.and_then(|slot| registration(slot, 1)))
            .collect::<Result<Vec<_>, _>>()?;
        for event_capacity in 1..=registration_count {
            let mut pending = KqueuePending::new(capacity)?;
            let mut deliveries = vec![0_usize; registration_count];
            for _ in 0..registration_count {
                add_all(&mut pending, &registrations)?;
                let selection = pending.delivery_selection(event_capacity);
                let selected = selection
                    .try_iter(pending.as_slice())?
                    .map(|entry| entry.registration)
                    .collect::<Vec<_>>();
                assert_eq!(selected.len(), event_capacity);
                assert!(selected.windows(2).all(|pair| pair[0] < pair[1]));
                for registration in selected {
                    let index = registrations
                        .iter()
                        .position(|candidate| *candidate == registration)
                        .ok_or(Error::Invariant)?;
                    deliveries[index] += 1;
                }
                pending.clear();
            }
            assert!(
                deliveries
                    .iter()
                    .all(|deliveries| *deliveries == event_capacity)
            );
        }
    }
    Ok(())
}

fn assert_delivery(
    pending: &mut KqueuePending,
    limit: usize,
    expected: &[crate::RegistrationId],
) -> Result<(), Error> {
    let selection = pending.delivery_selection(limit);
    let actual = selection
        .try_iter(pending.as_slice())?
        .map(|entry| entry.registration)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    Ok(())
}

fn add_all(
    pending: &mut KqueuePending,
    registrations: &[crate::RegistrationId],
) -> Result<(), Error> {
    for &registration in registrations {
        pending.add(registration, Key::ZERO, Readiness::READABLE)?;
    }
    Ok(())
}

fn registration(slot: u32, generation: u32) -> Result<crate::RegistrationId, Error> {
    let generation = NonZeroU32::new(generation).ok_or(Error::Invariant)?;
    let encoded = encode(slot, generation).ok_or(Error::Invariant)?;
    Ok(encoded.id())
}
