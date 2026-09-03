//! Post-delivery wait status.

use crate::RecoveryFailure;

/// Status returned after a valid event batch is delivered.
///
/// Process the delivered [`crate::Events`] before reconciling [`Self::recovery`].
/// Retrying a wait does not replace processing the current batch.
///
/// Ignoring the report is rejected when `unused_must_use` warnings are denied:
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use zio::{Poll, Wait};
///
/// # fn ignore_report() -> Result<(), zio::Error> {
/// let mut poll = Poll::new()?;
/// let mut events = poll.events()?;
/// poll.wait(&mut events, Wait::NoBlock)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
#[must_use = "process delivered events, then inspect the recovery report"]
pub struct WaitReport {
    recovery: Option<RecoveryFailure>,
}

impl WaitReport {
    pub(crate) const fn new(recovery: Option<RecoveryFailure>) -> Self {
        Self { recovery }
    }

    /// Returns whether the delivered batch needs no recovery reconciliation.
    pub const fn is_complete(&self) -> bool {
        self.recovery.is_none()
    }

    /// Borrows a post-delivery one-shot recovery failure, when present.
    pub const fn recovery(&self) -> Option<&RecoveryFailure> {
        self.recovery.as_ref()
    }

    /// Returns an owned post-delivery one-shot recovery failure, when present.
    pub fn into_recovery(self) -> Option<RecoveryFailure> {
        self.recovery
    }
}
