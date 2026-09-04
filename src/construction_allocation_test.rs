//! Construction allocation ceilings for the retained poller contract.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

use std::{error::Error as StdError, io};

use crate::{Key, Poll};

const CAPACITY: usize = 64;

#[test]
fn waker_normalized_construction_stays_compact() -> Result<(), Box<dyn StdError>> {
    let mut retained = None;

    let allocations = allocation_counter::measure(|| {
        retained = Some(
            Poll::builder()
                .event_capacity(CAPACITY)
                .registration_capacity(CAPACITY)
                .build()
                .and_then(|mut poll| {
                    let events = poll.events()?;
                    let waker = poll.waker(Key::new(1))?;
                    Ok((poll, events, waker))
                }),
        );
    });

    let retained = match retained {
        Some(Ok(retained)) => retained,
        Some(Err(error)) => return Err(Box::new(error)),
        None => return Err(io::Error::other("construction did not run").into()),
    };
    let expected_count = if cfg!(target_os = "linux") { 3 } else { 7 };
    assert_eq!(allocations.count_total, expected_count);
    assert_eq!(allocations.count_current, i64::try_from(expected_count)?);
    assert_eq!(allocations.count_max, expected_count);
    #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
    {
        let capacity = u64::try_from(CAPACITY)?;
        let expected_linux_bytes = 24 + capacity * 56;
        assert_eq!(allocations.bytes_total, expected_linux_bytes);
        assert_eq!(
            allocations.bytes_current,
            i64::try_from(expected_linux_bytes)?
        );
        assert_eq!(allocations.bytes_max, expected_linux_bytes);
    }
    #[cfg(target_os = "macos")]
    {
        let expected_macos_bytes = 14_616;
        assert_eq!(allocations.bytes_total, expected_macos_bytes);
        assert_eq!(
            allocations.bytes_current,
            i64::try_from(expected_macos_bytes)?
        );
        assert_eq!(allocations.bytes_max, expected_macos_bytes);
    }
    drop(retained);
    Ok(())
}
