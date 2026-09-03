//! Sparse kqueue observation coalescing and storage-reuse proofs.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

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
        let _ = pending.delivery_range(1);
        pending.clear();
    });

    result?;
    assert_eq!(allocations.count_total, 0);
    assert_eq!(allocations.bytes_total, 0);
    assert!(pending.as_slice().is_empty());
    Ok(())
}

#[test]
fn delivery_rotates_without_reordering_a_batch() -> Result<(), Error> {
    let capacity = NonZeroUsize::new(3).ok_or(Error::Invariant)?;
    let mut pending = KqueuePending::new(capacity)?;
    let registrations = [
        registration(0, 1)?,
        registration(1, 1)?,
        registration(2, 1)?,
    ];

    add_all(&mut pending, &registrations)?;
    assert_eq!(pending.delivery_range(2), 0..2);
    pending.clear();

    add_all(&mut pending, &registrations)?;
    assert_eq!(pending.delivery_range(2), 2..3);
    pending.clear();

    add_all(&mut pending, &registrations)?;
    assert_eq!(pending.delivery_range(2), 0..2);
    pending.clear();

    add_all(&mut pending, &registrations[1..])?;
    assert_eq!(pending.delivery_range(2), 0..2);
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
