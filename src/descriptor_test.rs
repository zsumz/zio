//! Compact descriptor retention regressions.

#![allow(
    unsafe_code,
    reason = "tests retain each source through the lifetime-erasing descriptor leaf"
)]

use std::{
    fs::File,
    mem::size_of,
    os::fd::{AsFd, AsRawFd},
};

use super::Descriptor;

#[test]
fn representation_remains_one_raw_descriptor() {
    assert_eq!(size_of::<Descriptor>(), size_of::<std::os::fd::RawFd>());
}

#[test]
fn borrowed_descriptor_preserves_caller_ownership() -> Result<(), std::io::Error> {
    let source = File::open("/dev/null")?;
    let expected = source.as_raw_fd();

    // SAFETY: `source` remains live and unchanged until `retained` is dropped.
    let retained = unsafe { Descriptor::borrowed(source.as_fd()) };
    assert_eq!(retained.as_raw_fd(), expected);
    assert_eq!(retained.as_fd().as_raw_fd(), expected);
    drop(retained);

    assert_eq!(source.metadata()?.len(), 0);
    Ok(())
}

#[test]
fn owned_descriptor_preserves_identity() -> Result<(), std::io::Error> {
    let source = File::open("/dev/null")?;
    let duplicate = source.as_fd().try_clone_to_owned()?;
    let expected = duplicate.as_raw_fd();

    let retained = Descriptor::owned(duplicate);
    assert_eq!(retained.as_raw_fd(), expected);
    assert_eq!(retained.as_fd().as_raw_fd(), expected);
    assert!(retained.is_owned());
    Ok(())
}
