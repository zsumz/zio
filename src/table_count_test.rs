//! Retained-registration count regressions.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use std::{error::Error as StdError, fs::File, num::NonZeroUsize, os::fd::AsFd};

use crate::{
    Error, Interest, Key, Mode, RegistrationId, descriptor::Descriptor, table::slot::FREE_END,
};

use super::RegistrationTable;

#[test]
fn count_tracks_fresh_reused_and_retired_slots() -> Result<(), Box<dyn StdError>> {
    let mut table = table(2)?;
    let source = File::open("/dev/null")?;
    assert_eq!(table.len(), 0);

    let first = reserve(&mut table, &source)?;
    let second = reserve(&mut table, &source)?;
    assert_eq!(table.len(), 2);

    retire(&mut table, first)?;
    assert_eq!(table.len(), 1);
    let reused = reserve(&mut table, &source)?;
    assert_eq!(table.len(), 2);

    retire(&mut table, second)?;
    retire(&mut table, reused)?;
    assert_eq!(table.len(), 0);
    Ok(())
}

#[test]
fn count_overflow_rejects_reservation_before_mutation() -> Result<(), Box<dyn StdError>> {
    let mut table = table(1)?;
    let source = File::open("/dev/null")?;
    table.live = usize::MAX;
    let permit = table.fresh_permit()?;
    let descriptor = Descriptor::owned(source.as_fd().try_clone_to_owned()?);
    let raw_descriptor = descriptor.as_raw_fd();

    let result = permit.reserve_with(
        descriptor,
        Key::ZERO,
        Interest::READABLE,
        Mode::Level,
        |_, _| (),
    );

    let Err(failure) = result else {
        return Err(Error::Invariant.into());
    };
    let (error, descriptor) = failure.into_parts();
    assert!(matches!(error, Error::Invariant));
    assert_eq!(descriptor.as_raw_fd(), raw_descriptor);
    assert!(table.slots.is_empty());
    assert_eq!(table.free_head, FREE_END);
    assert_eq!(table.live, usize::MAX);
    assert_eq!(table.remaining(), 0);
    Ok(())
}

#[test]
fn count_underflow_rejects_retirement_before_mutation() -> Result<(), Box<dyn StdError>> {
    let mut table = table(1)?;
    let source = File::open("/dev/null")?;
    let registration = reserve(&mut table, &source)?;
    table.live = 0;

    let result = retire(&mut table, registration);

    assert!(matches!(result, Err(Error::Invariant)));
    assert!(table.slots[0].entry.is_some());
    assert_eq!(table.free_head, FREE_END);
    assert_eq!(table.live, 0);
    Ok(())
}

fn table(capacity: usize) -> Result<RegistrationTable, Error> {
    RegistrationTable::new(NonZeroUsize::new(capacity).ok_or(Error::Invariant)?)
}

fn reserve(
    table: &mut RegistrationTable,
    source: &File,
) -> Result<RegistrationId, Box<dyn StdError>> {
    let descriptor = Descriptor::owned(source.as_fd().try_clone_to_owned()?);
    let (id, reservation) = if table.has_reusable_slot() {
        let permit = table.reused_permit()?;
        let id = permit.id();
        let (reservation, ()) = permit
            .reserve_with(
                descriptor,
                Key::ZERO,
                Interest::READABLE,
                Mode::Level,
                |_, _| (),
            )
            .map_err(super::permit::ReservationFailure::discard_descriptor)?;
        (id, reservation)
    } else {
        let permit = table.fresh_permit()?;
        let id = permit.id();
        let (reservation, ()) = permit
            .reserve_with(
                descriptor,
                Key::ZERO,
                Interest::READABLE,
                Mode::Level,
                |_, _| (),
            )
            .map_err(super::permit::ReservationFailure::discard_descriptor)?;
        (id, reservation)
    };
    Ok(reservation.keep(id))
}

fn retire(table: &mut RegistrationTable, id: RegistrationId) -> Result<(), Error> {
    table.prepare_retire(id, true)?.retire()
}
