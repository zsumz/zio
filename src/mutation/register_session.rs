//! Registration entrypoints over the shared mutation state.

use std::os::fd::{AsFd, OwnedFd};

use crate::{
    Error, Interest, Key, Mode, Operation, RegisterError, Registration, descriptor::Descriptor,
};

use super::{
    MutationDriver, MutationSession,
    register::{RegisterFailure, register_descriptor as apply_registration},
};

impl<Driver: MutationDriver> MutationSession<'_, Driver> {
    pub(crate) fn register<F: AsFd + ?Sized>(
        &mut self,
        source: &F,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        validate_interest(interest).map_err(|error| RegisterError::new(error, None))?;
        let descriptor = source.as_fd().try_clone_to_owned().map_err(|source| {
            RegisterError::new(
                Error::Io {
                    operation: Operation::Register,
                    source,
                },
                None,
            )
        })?;
        self.register_descriptor(Descriptor::owned(descriptor), key, interest, mode)
            .map_err(RegisterFailure::discard_released)
    }

    pub(crate) fn register_owned(
        &mut self,
        source: OwnedFd,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterFailure> {
        let descriptor = Descriptor::owned(source);
        if let Err(error) = validate_interest(interest) {
            return Err(RegisterFailure::released(error, descriptor));
        }
        self.register_descriptor(descriptor, key, interest, mode)
    }

    /// Registers after erasing the source borrow into the retained table.
    ///
    /// # Safety
    ///
    /// The source must satisfy the complete lifetime and identity contract of
    /// [`crate::Poll::register_borrowed`].
    #[allow(
        unsafe_code,
        reason = "this internal seam propagates the public borrowed-descriptor contract"
    )]
    #[inline]
    pub(crate) unsafe fn register_borrowed<F: AsFd + ?Sized>(
        &mut self,
        source: &F,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        validate_interest(interest).map_err(|error| RegisterError::new(error, None))?;
        // SAFETY: this function requires the caller to uphold the descriptor
        // lifetime and identity invariant until the retained value is dropped.
        let descriptor = unsafe { Descriptor::borrowed(source.as_fd()) };
        self.register_descriptor(descriptor, key, interest, mode)
            .map_err(RegisterFailure::discard_released)
    }

    #[inline]
    fn register_descriptor(
        &mut self,
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterFailure> {
        apply_registration(
            self.owner,
            self.registrations,
            &mut *self.driver,
            descriptor,
            key,
            interest,
            mode,
        )
    }
}

#[inline]
fn validate_interest(interest: Interest) -> Result<(), Error> {
    (!interest.is_empty())
        .then_some(())
        .ok_or(Error::InvalidInterest)
}
