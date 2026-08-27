//! Sparse kqueue observation coalescing and storage-reuse proofs.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use core::num::{NonZeroU32, NonZeroUsize};

use crate::{Error, Key, Readiness, pending_kqueue::KqueuePending, token::encode};

#[test]
fn sparse_clear_reuses_storage_and_resets_generation_guard() -> Result<(), Error> {
    let capacity = NonZeroUsize::new(4).ok_or(Error::Invariant)?;
    let registrations = NonZeroUsize::new(8).ok_or(Error::Invariant)?;
    let mut pending = KqueuePending::new(capacity, registrations)?;
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
    let mut pending = KqueuePending::new(capacity, capacity)?;
    let registration = registration(1, 1)?;
    let mut result = Ok(());

    let allocations = allocation_counter::measure(|| {
        result = pending.add(registration, Key::new(9), Readiness::READABLE);
        if result.is_ok() {
            result = pending.add(registration, Key::new(9), Readiness::WRITABLE);
        }
        pending.clear();
    });

    result?;
    assert_eq!(allocations.count_total, 0);
    assert_eq!(allocations.bytes_total, 0);
    assert!(pending.as_slice().is_empty());
    Ok(())
}

fn registration(slot: u32, generation: u32) -> Result<crate::RegistrationId, Error> {
    let generation = NonZeroU32::new(generation).ok_or(Error::Invariant)?;
    let encoded = encode(slot, generation).ok_or(Error::Invariant)?;
    Ok(encoded.id())
}
