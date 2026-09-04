//! Validation and disjoint access for the intrusive FIFO free queue.

use crate::{Error, descriptor::Descriptor, token::MAX_GENERATION};

use super::slot::{FREE_END, Slot};

pub(super) fn release_slot(
    slots: &mut [Slot],
    slot_index: usize,
    free_head: &mut u32,
    free_tail: &mut u32,
    exhausted: &mut usize,
    live: &mut usize,
    free_index: u32,
) -> Result<Descriptor, Error> {
    if usize::try_from(free_index).ok() != Some(slot_index) {
        return Err(Error::Invariant);
    }
    let next_live = live.checked_sub(1).ok_or(Error::Invariant)?;
    let slot = slots.get(slot_index).ok_or(Error::Invariant)?;
    if slot.entry.is_none() {
        return Err(Error::Invariant);
    }
    let next_exhausted = if slot.generation == MAX_GENERATION {
        Some(exhausted.checked_add(1).ok_or(Error::Invariant)?)
    } else {
        None
    };
    let tail_index = if next_exhausted.is_none() {
        validate_free_queue(slots, slot_index, *free_head, *free_tail)?
    } else {
        None
    };
    let entry = match (next_exhausted, tail_index) {
        (Some(next_exhausted), _) => {
            let slot = slots.get_mut(slot_index).ok_or(Error::Invariant)?;
            let entry = slot.entry.take().ok_or(Error::Invariant)?;
            slot.next_free = FREE_END;
            *exhausted = next_exhausted;
            entry
        }
        (None, None) => {
            let slot = slots.get_mut(slot_index).ok_or(Error::Invariant)?;
            let entry = slot.entry.take().ok_or(Error::Invariant)?;
            slot.next_free = FREE_END;
            *free_head = free_index;
            *free_tail = free_index;
            entry
        }
        (None, Some(tail_index)) => {
            let (slot, tail) = two_slots_mut(slots, slot_index, tail_index)?;
            let entry = slot.entry.take().ok_or(Error::Invariant)?;
            slot.next_free = FREE_END;
            tail.next_free = free_index;
            *free_tail = free_index;
            entry
        }
    };
    *live = next_live;
    Ok(entry.descriptor)
}

fn validate_free_queue(
    slots: &[Slot],
    occupied: usize,
    free_head: u32,
    free_tail: u32,
) -> Result<Option<usize>, Error> {
    if (free_head == FREE_END) != (free_tail == FREE_END) {
        return Err(Error::Invariant);
    }
    if free_tail == FREE_END {
        return Ok(None);
    }
    for free_index in [free_head, free_tail] {
        let index = usize::try_from(free_index).map_err(|_| Error::Invariant)?;
        let slot = slots.get(index).ok_or(Error::Invariant)?;
        if index == occupied || slot.entry.is_some() || slot.generation == MAX_GENERATION {
            return Err(Error::Invariant);
        }
    }
    let index = usize::try_from(free_tail).map_err(|_| Error::Invariant)?;
    if slots.get(index).ok_or(Error::Invariant)?.next_free != FREE_END {
        return Err(Error::Invariant);
    }
    Ok(Some(index))
}

fn two_slots_mut(
    slots: &mut [Slot],
    slot_index: usize,
    tail_index: usize,
) -> Result<(&mut Slot, &mut Slot), Error> {
    if slot_index == tail_index {
        return Err(Error::Invariant);
    }
    if slot_index < tail_index {
        let (before_tail, tail) = slots.split_at_mut(tail_index);
        let slot = before_tail.get_mut(slot_index).ok_or(Error::Invariant)?;
        let tail = tail.first_mut().ok_or(Error::Invariant)?;
        Ok((slot, tail))
    } else {
        let (before_slot, slot) = slots.split_at_mut(slot_index);
        let tail = before_slot.get_mut(tail_index).ok_or(Error::Invariant)?;
        let slot = slot.first_mut().ok_or(Error::Invariant)?;
        Ok((slot, tail))
    }
}
