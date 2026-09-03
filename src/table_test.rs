//! Fixed registration table allocation and reuse regressions.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use std::{
    error::Error as StdError,
    fs::File,
    mem::size_of,
    num::NonZeroUsize,
    os::fd::{AsFd, AsRawFd, OwnedFd},
};

use crate::{
    Error, Interest, Key, Mode, RegistrationId,
    descriptor::Descriptor,
    token::{MAX_GENERATION, decode},
};

use super::{RegistrationTable, slot::Slot};

impl RegistrationTable {
    pub(crate) fn reserve_descriptor(
        &mut self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<RegistrationId, Error> {
        let (id, reservation) = if self.has_reusable_slot() {
            let permit = self.reused_permit()?;
            let id = permit.id();
            let (reservation, ()) =
                permit.reserve_with(descriptor, key, interest, mode, |_, _| ())?;
            (id, reservation)
        } else {
            let permit = self.fresh_permit()?;
            let id = permit.id();
            let (reservation, ()) =
                permit.reserve_with(descriptor, key, interest, mode, |_, _| ())?;
            (id, reservation)
        };
        Ok(reservation.keep(id))
    }

    pub(crate) fn reserve(
        &mut self,
        descriptor: OwnedFd,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<RegistrationId, Error> {
        self.reserve_descriptor(Descriptor::owned(descriptor), key, interest, mode)
    }

    pub(crate) fn retire(&mut self, id: RegistrationId) -> Result<(), Error> {
        self.prepare_retire(id, true)?.retire()
    }
}

#[test]
#[cfg(target_pointer_width = "64")]
fn slot_layout_remains_compact_on_supported_targets() {
    assert_eq!(size_of::<Slot>(), 24);
}

#[test]
fn construction_allocates_only_the_slot_buffer() -> Result<(), Box<dyn StdError>> {
    let limit = NonZeroUsize::new(1_024).ok_or(Error::Invariant)?;
    let mut result = None;

    let allocations = allocation_counter::measure(|| {
        result = Some(RegistrationTable::new(limit));
    });

    let table = match result {
        Some(Ok(table)) => table,
        Some(Err(error)) => return Err(Box::new(error)),
        None => return Err(Box::new(Error::Invariant)),
    };
    let expected_bytes = u64::try_from(size_of::<Slot>() * limit.get())?;
    assert_eq!(allocations.count_total, 1);
    assert_eq!(allocations.count_current, 1);
    assert_eq!(allocations.count_max, 1);
    assert_eq!(allocations.bytes_total, expected_bytes);
    assert_eq!(allocations.bytes_current, i64::try_from(expected_bytes)?);
    assert_eq!(allocations.bytes_max, expected_bytes);
    assert_eq!(table.slots.capacity(), limit.get());
    assert!(table.slots.is_empty());
    Ok(())
}

#[test]
fn reserve_and_retire_remain_allocation_free() -> Result<(), Box<dyn StdError>> {
    let mut table = table(1)?;
    let source = File::open("/dev/null")?;
    let descriptor = source.as_fd().try_clone_to_owned()?;
    let mut result = None;

    let allocations = allocation_counter::measure(|| {
        result = Some(
            table
                .reserve(descriptor, Key::new(1), Interest::READABLE, Mode::Level)
                .and_then(|id| {
                    table.retire(id)?;
                    Ok(id)
                }),
        );
    });

    let retired = match result {
        Some(Ok(id)) => id,
        Some(Err(error)) => return Err(Box::new(error)),
        None => return Err(Box::new(Error::Invariant)),
    };
    assert_eq!(allocations.count_total, 0);
    assert_eq!(allocations.count_current, 0);
    assert_eq!(allocations.count_max, 0);

    let reused = reserve(&mut table, &source, Key::new(2))?;
    let (retired_index, retired_generation) = decode(retired)?;
    let (reused_index, reused_generation) = decode(reused)?;
    assert_eq!(reused_index, retired_index);
    assert_eq!(reused_generation.get(), retired_generation.get() + 1);
    Ok(())
}

#[test]
fn reservation_carries_the_inserted_descriptor_and_retire_proof() -> Result<(), Box<dyn StdError>> {
    let mut table = table(1)?;
    let source = File::open("/dev/null")?;
    let descriptor = source.as_fd().try_clone_to_owned()?;
    let raw_descriptor = descriptor.as_raw_fd();
    let permit = table.fresh_permit()?;
    let id = permit.id();
    let (reservation, observed) = permit.reserve_with(
        Descriptor::owned(descriptor),
        Key::new(1),
        Interest::READABLE,
        Mode::Level,
        |descriptor, id| (descriptor.as_raw_fd(), id),
    )?;
    assert_eq!(observed, (raw_descriptor, id));
    reservation.retire()?;
    assert!(matches!(
        table.state(id),
        Err(Error::Stale { registration }) if registration == id
    ));
    Ok(())
}

#[test]
fn retired_slots_are_reused_in_lifo_order() -> Result<(), Box<dyn StdError>> {
    let mut table = table(3)?;
    let source = File::open("/dev/null")?;
    let first = reserve(&mut table, &source, Key::new(1))?;
    let second = reserve(&mut table, &source, Key::new(2))?;
    let _third = reserve(&mut table, &source, Key::new(3))?;

    table.retire(second)?;
    table.retire(first)?;
    let first_reused = reserve(&mut table, &source, Key::new(4))?;
    let second_reused = reserve(&mut table, &source, Key::new(5))?;

    assert_reused(first, first_reused)?;
    assert_reused(second, second_reused)?;
    Ok(())
}

#[test]
fn permanent_exhaustion_is_distinct_from_live_capacity() -> Result<(), Box<dyn StdError>> {
    let mut table = table(2)?;
    let source = File::open("/dev/null")?;
    let seeded_first = reserve(&mut table, &source, Key::new(1))?;
    let seeded_second = reserve(&mut table, &source, Key::new(2))?;
    table.retire(seeded_first)?;
    table.retire(seeded_second)?;
    table.slots[0].generation = MAX_GENERATION - 1;
    table.slots[1].generation = MAX_GENERATION - 1;
    let first = reserve(&mut table, &source, Key::new(3))?;
    let second = reserve(&mut table, &source, Key::new(4))?;

    assert_capacity(&reserve(&mut table, &source, Key::new(5)), 2);
    table.retire(first)?;
    assert_capacity(&reserve(&mut table, &source, Key::new(6)), 2);
    table.retire(second)?;
    assert!(matches!(
        reserve(&mut table, &source, Key::new(7)),
        Err(Error::RegistrationSpaceExhausted)
    ));
    assert_eq!(table.exhausted, 2);
    assert_eq!(table.remaining(), 0);
    Ok(())
}

#[test]
fn virgin_capacity_remains_after_an_initialized_slot_exhausts() -> Result<(), Box<dyn StdError>> {
    let mut table = table(2)?;
    let source = File::open("/dev/null")?;
    let seeded = reserve(&mut table, &source, Key::new(1))?;
    table.retire(seeded)?;
    table.slots[0].generation = MAX_GENERATION - 1;
    let exhausted = reserve(&mut table, &source, Key::new(2))?;
    table.retire(exhausted)?;

    assert_eq!(table.remaining(), 1);

    let registration = reserve(&mut table, &source, Key::new(3))?;
    let (index, generation) = decode(registration)?;
    assert_eq!(index, 1);
    assert_eq!(generation.get(), 1);
    assert_eq!(table.exhausted, 1);
    Ok(())
}

fn table(capacity: usize) -> Result<RegistrationTable, Error> {
    RegistrationTable::new(NonZeroUsize::new(capacity).ok_or(Error::Invariant)?)
}

fn reserve(
    table: &mut RegistrationTable,
    source: &File,
    key: Key,
) -> Result<crate::RegistrationId, Error> {
    let descriptor = source
        .as_fd()
        .try_clone_to_owned()
        .map_err(|source| Error::Io {
            operation: crate::Operation::Register,
            source,
        })?;
    table.reserve(descriptor, key, Interest::READABLE, Mode::Level)
}

fn assert_reused(
    retired: crate::RegistrationId,
    reused: crate::RegistrationId,
) -> Result<(), Error> {
    let (retired_index, retired_generation) = decode(retired)?;
    let (reused_index, reused_generation) = decode(reused)?;
    assert_eq!(reused_index, retired_index);
    assert_eq!(reused_generation.get(), retired_generation.get() + 1);
    Ok(())
}

fn assert_capacity(result: &Result<crate::RegistrationId, Error>, expected: usize) {
    assert!(matches!(result, Err(Error::Capacity { limit }) if *limit == expected));
}
