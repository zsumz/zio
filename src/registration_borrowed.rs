//! Explicit borrowed-descriptor registration tier.

use std::os::fd::AsFd;

use crate::{Interest, Key, Mode, Poll, RegisterError, Registration, mutation::MutationSession};

impl Poll {
    /// Registers one descriptor without duplicating or owning it.
    ///
    /// This expert tier preserves the same poller authority, exact generation,
    /// mutation outcomes, readiness behavior, and explicit deletion contract as
    /// [`Self::register`]. It avoids the duplicate-descriptor syscall and keeps
    /// the caller's descriptor count unchanged.
    ///
    /// # Safety
    ///
    /// `source`'s numeric descriptor must remain open and identify the same
    /// open-file description throughout this call. After success or an error
    /// carrying a [`Registration`], it must remain so until deletion is proven
    /// applied or this poller is dropped. A returned error for which
    /// [`RegisterError::registration`] is `None` ends the obligation when this
    /// call returns; this covers preflight and proven not-applied failures.
    ///
    /// The continuing obligation survives copying or dropping every handle,
    /// one-shot disarming, unwinding, and any later mutation failure that is not
    /// proven applied deletion.
    ///
    /// At most one live borrowed registration for a numeric descriptor may
    /// exist in this poller. Duplicate the descriptor before registering it
    /// independently. Do not concurrently close, replace, or reassign the
    /// registered descriptor. Explicit successful deletion is required when
    /// deterministic native cleanup matters; dropping the poller only ends
    /// zio's ability to mutate or deliver the registration.
    #[allow(
        unsafe_code,
        reason = "the caller explicitly assumes the borrowed descriptor lifetime contract"
    )]
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
