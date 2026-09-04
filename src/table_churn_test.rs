//! Sustained registration-churn distribution regression.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use std::{error::Error as StdError, fs::File, num::NonZeroUsize, os::fd::AsFd};

use crate::{Interest, Key, Mode, token::decode};

use super::RegistrationTable;

#[test]
fn sustained_single_registration_churn_rotates_evenly() -> Result<(), Box<dyn StdError>> {
    const CAPACITY: usize = 64;
    const FULL_ROTATIONS: usize = 128;

    let mut table =
        RegistrationTable::new(NonZeroUsize::new(CAPACITY).ok_or("capacity must be non-zero")?)?;
    let source = File::open("/dev/null")?;
    let mut final_generations = [0_u32; CAPACITY];

    for cycle in 0..CAPACITY * FULL_ROTATIONS {
        let descriptor = source.as_fd().try_clone_to_owned()?;
        let registration = table.reserve(
            descriptor,
            Key::new(u64::try_from(cycle)?),
            Interest::READABLE,
            Mode::Level,
        )?;
        let (slot, generation) = decode(registration)?;

        assert_eq!(slot, cycle % CAPACITY);
        final_generations[slot] = generation.get();
        table.retire(registration)?;
    }

    let expected_generation = u32::try_from(FULL_ROTATIONS)?;
    assert_eq!(final_generations, [expected_generation; CAPACITY]);
    assert_eq!(table.len(), 0);
    assert_eq!(table.remaining(), CAPACITY);
    Ok(())
}
