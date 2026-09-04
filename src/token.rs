//! Nonzero native tokens for exact slot generations.

use core::{fmt, num::NonZeroU32};

use crate::{Error, RegistrationId};

pub(crate) const MAX_REGISTRATIONS: usize = u32::MAX as usize;
pub(crate) const MAX_GENERATION: u32 = u32::MAX;

/// Internally proven encoding retained only by library-created registrations.
#[repr(transparent)]
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct EncodedRegistrationId(RegistrationId);

impl EncodedRegistrationId {
    pub(crate) const fn from_verified(id: RegistrationId) -> Self {
        Self(id)
    }

    pub(crate) const fn id(self) -> RegistrationId {
        self.0
    }

    #[inline]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the private constructor packs one u32 into each exact token half"
    )]
    pub(crate) const fn parts(self) -> (u32, u32) {
        let token = self.0.get();
        let slot = token as u32;
        let generation = (token >> u32::BITS) as u32;
        (slot.wrapping_sub(1), generation)
    }
}

impl fmt::Debug for EncodedRegistrationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, formatter)
    }
}

pub(crate) fn encode(index: u32, generation: NonZeroU32) -> Option<EncodedRegistrationId> {
    let slot = index.checked_add(1)?;
    Some(EncodedRegistrationId::from_verified(RegistrationId::new(
        (u64::from(generation.get()) << u32::BITS) | u64::from(slot),
    )))
}

#[inline]
pub(crate) fn decode(id: RegistrationId) -> Result<(usize, NonZeroU32), Error> {
    let token = id.get();
    if token == 0 {
        return Err(Error::Stale { registration: id });
    }
    let slot = u32::try_from(token & u64::from(u32::MAX)).map_err(|_| Error::Invariant)?;
    let generation = u32::try_from(token >> u32::BITS).map_err(|_| Error::Invariant)?;
    let index = slot
        .checked_sub(1)
        .ok_or(Error::Stale { registration: id })?;
    Ok((
        usize::try_from(index).map_err(|_| Error::Invariant)?,
        NonZeroU32::new(generation).ok_or(Error::Stale { registration: id })?,
    ))
}

#[cfg(test)]
#[path = "token_test.rs"]
mod tests;
