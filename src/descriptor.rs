//! Compact owned-or-borrowed descriptor retention.

#![allow(
    unsafe_code,
    reason = "raw descriptor reconstruction is confined to this reviewed lifetime leaf"
)]

use std::{
    fmt,
    os::fd::{AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
};

use crate::DescriptorOwnership;

const BORROWED_TAG: RawFd = RawFd::MIN;
const DESCRIPTOR_BITS: RawFd = RawFd::MAX;

/// One descriptor plus its close-on-drop policy, encoded in one `RawFd`.
#[repr(transparent)]
pub(crate) struct Descriptor(RawFd);

impl Descriptor {
    pub(crate) fn owned(descriptor: OwnedFd) -> Self {
        Self(descriptor.into_raw_fd())
    }

    /// Erases the borrow lifetime while retaining the descriptor identity.
    ///
    /// # Safety
    ///
    /// `descriptor` must remain valid and identify the same open-file
    /// description until this value is dropped.
    pub(crate) unsafe fn borrowed(descriptor: BorrowedFd<'_>) -> Self {
        Self(descriptor.as_raw_fd() | BORROWED_TAG)
    }

    #[inline]
    pub(crate) fn as_fd(&self) -> BorrowedFd<'_> {
        // SAFETY: owned values consume and retain an `OwnedFd`. Borrowed values
        // are created only behind the public unsafe registration contract,
        // which requires this exact descriptor to remain live until retirement.
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }

    pub(crate) const fn as_raw_fd(&self) -> RawFd {
        self.0 & DESCRIPTOR_BITS
    }

    const fn is_owned(&self) -> bool {
        self.0 >= 0
    }

    pub(crate) const fn ownership(&self) -> DescriptorOwnership {
        if self.is_owned() {
            DescriptorOwnership::Owned
        } else {
            DescriptorOwnership::Borrowed
        }
    }

    /// Reconstructs the descriptor consumed by [`Self::owned`].
    ///
    /// # Panics
    ///
    /// Panics if this descriptor is borrowed.
    pub(crate) fn into_owned(self) -> OwnedFd {
        assert!(self.is_owned(), "borrowed descriptor cannot become owned");
        let descriptor = std::mem::ManuallyDrop::new(self);
        // SAFETY: the ownership tag proves that `owned` consumed this exact
        // descriptor. `ManuallyDrop` prevents closing it twice.
        unsafe { OwnedFd::from_raw_fd(descriptor.as_raw_fd()) }
    }
}

impl fmt::Debug for Descriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Descriptor")
            .field("raw", &self.as_raw_fd())
            .field("owned", &self.is_owned())
            .finish()
    }
}

impl Drop for Descriptor {
    #[inline]
    fn drop(&mut self) {
        if self.is_owned() {
            // SAFETY: `owned` consumed this exact `OwnedFd`, and the encoded
            // value was never exposed as another owner. Reconstruction returns
            // its single close obligation to `OwnedFd` exactly once.
            drop(unsafe { OwnedFd::from_raw_fd(self.as_raw_fd()) });
        }
    }
}

#[cfg(test)]
#[path = "descriptor_test.rs"]
mod tests;
