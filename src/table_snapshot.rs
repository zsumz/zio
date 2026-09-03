//! Registration iteration and snapshot construction.

use std::{iter::FusedIterator, num::NonZeroU32};

use crate::{
    CapacityKind, CapacityReason, Error, Registration, RegistrationId, registration::PollId,
    token::encode,
};

use super::{RegistrationTable, slot::Slot};

/// Validated allocation-free view over retained registrations.
pub(crate) struct RegistrationIter<'table> {
    inner: RegistrationIterInner<'table>,
}

enum RegistrationIterInner<'table> {
    Empty,
    Retained {
        owner: PollId,
        slots: &'table [Slot],
        front: usize,
        back: usize,
        remaining: usize,
    },
}

impl RegistrationTable {
    pub(crate) fn registration_iter(
        &self,
        owner: Option<PollId>,
    ) -> Result<RegistrationIter<'_>, Error> {
        let mut occupied = 0_usize;
        for (index, slot) in self.slots.iter().enumerate() {
            if slot.entry.is_none() {
                continue;
            }
            validate_registration(index, slot)?;
            occupied = occupied.checked_add(1).ok_or(Error::Invariant)?;
        }
        if occupied != self.live {
            return Err(Error::Invariant);
        }
        if occupied == 0 {
            return Ok(RegistrationIter {
                inner: RegistrationIterInner::Empty,
            });
        }
        let owner = owner.ok_or(Error::Invariant)?;
        Ok(RegistrationIter {
            inner: RegistrationIterInner::Retained {
                owner,
                slots: &self.slots,
                front: 0,
                back: self.slots.len(),
                remaining: occupied,
            },
        })
    }

    pub(crate) fn snapshot(&self, owner: PollId) -> Result<Vec<Registration>, Error> {
        let retained = self.registration_iter(Some(owner))?;
        let occupied = retained.len();
        let mut registrations = Vec::new();
        registrations
            .try_reserve_exact(occupied)
            .map_err(|_| Error::Capacity {
                kind: CapacityKind::Registration,
                limit: occupied,
                reason: CapacityReason::StorageUnavailable,
            })?;
        registrations.extend(retained);
        if registrations.len() != occupied {
            return Err(Error::Invariant);
        }
        Ok(registrations)
    }
}

impl Iterator for RegistrationIter<'_> {
    type Item = Registration;

    fn next(&mut self) -> Option<Self::Item> {
        let RegistrationIterInner::Retained {
            owner,
            slots,
            front,
            back,
            remaining,
        } = &mut self.inner
        else {
            return None;
        };
        while *front < *back {
            let index = *front;
            *front += 1;
            let slot = &slots[index];
            if slot.entry.is_none() {
                continue;
            }
            *remaining -= 1;
            return Some(validated_registration(*owner, index, slot));
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.len();
        (remaining, Some(remaining))
    }
}

impl DoubleEndedIterator for RegistrationIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let RegistrationIterInner::Retained {
            owner,
            slots,
            front,
            back,
            remaining,
        } = &mut self.inner
        else {
            return None;
        };
        while *front < *back {
            *back -= 1;
            let index = *back;
            let slot = &slots[index];
            if slot.entry.is_none() {
                continue;
            }
            *remaining -= 1;
            return Some(validated_registration(*owner, index, slot));
        }
        None
    }
}

impl ExactSizeIterator for RegistrationIter<'_> {
    fn len(&self) -> usize {
        match self {
            Self {
                inner: RegistrationIterInner::Empty,
            } => 0,
            Self {
                inner: RegistrationIterInner::Retained { remaining, .. },
            } => *remaining,
        }
    }
}

impl FusedIterator for RegistrationIter<'_> {}

fn validate_registration(index: usize, slot: &Slot) -> Result<(), Error> {
    let index = u32::try_from(index).map_err(|_| Error::Invariant)?;
    let generation = NonZeroU32::new(slot.generation).ok_or(Error::Invariant)?;
    let _ = encode(index, generation).ok_or(Error::Invariant)?;
    Ok(())
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "iterator construction proves every occupied index fits u32"
)]
fn validated_registration(owner: PollId, index: usize, slot: &Slot) -> Registration {
    let slot_number = index as u64 + 1;
    let raw = (u64::from(slot.generation) << u32::BITS) | slot_number;
    Registration::from_verified(owner, RegistrationId::new(raw))
}
