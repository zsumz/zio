//! Nonzero native tokens for exact slot generations.

use core::num::NonZeroU32;

use crate::{Error, RegistrationId};

pub(crate) const MAX_REGISTRATIONS: usize = u32::MAX as usize;
pub(crate) const MAX_GENERATION: u32 = u32::MAX;

pub(crate) fn encode(index: u32, generation: NonZeroU32) -> Option<RegistrationId> {
    let slot = u64::from(index).checked_add(1)?;
    Some(RegistrationId::new(
        (u64::from(generation.get()) << u32::BITS) | slot,
    ))
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
