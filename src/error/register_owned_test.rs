//! Owned-registration error regressions.

use std::{
    fs::File,
    os::fd::{AsRawFd, OwnedFd},
};

use crate::{Error, Registration};

use super::RegisterOwnedError;

#[test]
fn returned_error_exposes_the_exact_descriptor() -> Result<(), std::io::Error> {
    let descriptor: OwnedFd = File::open("/dev/null")?.into();
    let raw = descriptor.as_raw_fd();
    let error = RegisterOwnedError::returned(Error::InvalidInterest, descriptor);

    assert!(matches!(error.error(), Error::InvalidInterest));
    assert_eq!(error.descriptor().map(AsRawFd::as_raw_fd), Some(raw));
    assert_eq!(error.registration(), None);
    assert!(core::ptr::eq(error.error(), error.as_ref()));
    Ok(())
}

#[test]
fn retained_error_exposes_the_exact_registration() {
    let registration = Registration::test(7);
    let error = RegisterOwnedError::retained(Error::Invariant, registration);

    assert!(matches!(error.error(), Error::Invariant));
    assert!(error.descriptor().is_none());
    assert_eq!(error.registration(), Some(registration));
    assert!(core::ptr::eq(error.error(), error.as_ref()));
}
