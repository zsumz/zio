//! Reused-slot insertion and free-link regressions.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use std::{error::Error as StdError, fs::File, num::NonZeroUsize, os::fd::AsFd};

use crate::{
    Error, Interest, Key, Mode, RegistrationId, RegistrationState, descriptor::Descriptor,
    token::decode,
};

use super::{RegistrationTable, slot::FREE_END};

#[test]
fn corrupted_reused_slot_is_rejected_without_table_mutation() -> Result<(), Box<dyn StdError>> {
    let mut table = table(1)?;
    let source = File::open("/dev/null")?;
    let original = reserve(&mut table, &source, Key::new(1))?;
    let original_descriptor = table.slots[0]
        .entry
        .as_ref()
        .ok_or(Error::Invariant)?
        .descriptor
        .as_raw_fd();
    let generation = table.slots[0].generation;
    let next_free = table.slots[0].next_free;
    table.free_head = 0;

    let result = table.check_reservable();

    assert!(matches!(result, Err(Error::Invariant)));
    assert_eq!(table.free_head, 0);
    assert_eq!(table.slots[0].generation, generation);
    assert_eq!(table.slots[0].next_free, next_free);
    let entry = table.slots[0].entry.as_ref().ok_or(Error::Invariant)?;
    assert_eq!(entry.descriptor.as_raw_fd(), original_descriptor);
    assert_eq!(entry.key, Key::new(1));
    assert_eq!(
        table.state(original)?,
        RegistrationState::Registered {
            arm: crate::ArmState::Armed,
        }
    );
    Ok(())
}

#[test]
fn retire_overwrites_stale_occupied_free_link() -> Result<(), Box<dyn StdError>> {
    let mut table = table(2)?;
    let source = File::open("/dev/null")?;
    let first = reserve(&mut table, &source, Key::new(1))?;
    let second = reserve(&mut table, &source, Key::new(2))?;
    retire(&mut table, first)?;
    retire(&mut table, second)?;
    let first_index = index(first)?;
    let second_index = index(second)?;

    let reused_second = reserve(&mut table, &source, Key::new(3))?;
    assert_eq!(
        table.slots[second_index].next_free,
        u32::try_from(first_index)?
    );
    let reused_first = reserve(&mut table, &source, Key::new(4))?;
    assert_eq!(table.free_head, FREE_END);

    retire(&mut table, reused_second)?;
    assert_eq!(table.slots[second_index].next_free, FREE_END);
    retire(&mut table, reused_first)?;
    assert_eq!(
        table.slots[first_index].next_free,
        u32::try_from(second_index)?
    );

    assert_eq!(
        index(reserve(&mut table, &source, Key::new(5))?)?,
        first_index
    );
    assert_eq!(
        index(reserve(&mut table, &source, Key::new(6))?)?,
        second_index
    );
    Ok(())
}

fn table(capacity: usize) -> Result<RegistrationTable, Error> {
    RegistrationTable::new(NonZeroUsize::new(capacity).ok_or(Error::Invariant)?)
}

fn reserve(
    table: &mut RegistrationTable,
    source: &File,
    key: Key,
) -> Result<RegistrationId, Box<dyn StdError>> {
    let descriptor = source.as_fd().try_clone_to_owned()?;
    let permit = table.check_reservable()?;
    let id = permit.id();
    let reservation = permit.reserve(
        Descriptor::owned(descriptor),
        key,
        Interest::READABLE,
        Mode::Level,
    )?;
    Ok(reservation.keep(id))
}

fn retire(table: &mut RegistrationTable, id: RegistrationId) -> Result<(), Error> {
    table.prepare_retire(id, true)?.retire()
}

fn index(id: RegistrationId) -> Result<usize, Error> {
    decode(id).map(|(index, _)| index)
}
