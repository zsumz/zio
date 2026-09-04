//! Explicit borrowed-descriptor registration tier.

use std::os::fd::AsFd;

use crate::{Interest, Key, Mode, Poll, RegisterError, Registration, mutation::MutationSession};

impl Poll {
    /// Registers one descriptor with non-empty interest without duplicating or owning it.
    ///
    /// This expert tier follows [`Self::register`] semantics without a
    /// duplicate-descriptor syscall or ownership transfer.
    ///
    /// # Safety
    ///
    /// The caller must:
    ///
    /// - keep the numeric descriptor open and bound to the same open-file
    ///   description throughout this call;
    /// - after success or an error carrying a [`Registration`], maintain that
    ///   identity until deletion is proven applied or this poller is dropped;
    /// - never concurrently close, replace, or reassign the descriptor; and
    /// - keep at most one live borrowed registration for that numeric
    ///   descriptor in this poller. Duplicate it for independent registration.
    ///
    /// [`RegisterError::registration`] returning `None` ends the obligation on
    /// return. Copying or dropping handles, disarming, unwinding, and any later
    /// mutation without proven deletion do not.
    ///
    /// Use explicit deletion when deterministic native cleanup matters. Poller
    /// drop only stops zio from mutating or delivering the registration.
    #[allow(
        unsafe_code,
        reason = "the caller explicitly assumes the borrowed descriptor lifetime contract"
    )]
    #[inline]
    pub unsafe fn register_borrowed<F: AsFd + ?Sized>(
        &mut self,
        source: &F,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<Registration, RegisterError> {
        // SAFETY: this public unsafe boundary passes its complete source
        // lifetime and identity obligation to the retained mutation seam.
        unsafe {
            MutationSession::new(&mut self.owner, &mut self.registrations, &mut self.backend)
                .register_borrowed(source, key, interest, mode)
        }
    }
}
