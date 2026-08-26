//! Intrusive registration slot representation.

use crate::{ArmState, Interest, Key, Mode, RegistrationState, descriptor::Descriptor};

// A capacity is a count, so its largest valid index is always below this value.
pub(super) const FREE_END: u32 = u32::MAX;

#[derive(Debug)]
pub(super) struct Entry {
    pub(super) descriptor: Descriptor,
    pub(super) key: Key,
    pub(super) interest: Interest,
    pub(super) mode: Mode,
    pub(super) state: RegistrationState,
}

impl Entry {
    pub(super) const fn registered(
        descriptor: Descriptor,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Self {
        Self {
            descriptor,
            key,
            interest,
            mode,
            state: RegistrationState::Registered {
                arm: ArmState::Armed,
            },
        }
    }
}

#[derive(Debug)]
pub(super) struct Slot {
    pub(super) generation: u32,
    pub(super) entry: Option<Entry>,
    pub(super) next_free: u32,
}

impl Slot {
    pub(super) const fn occupied(generation: u32, entry: Entry) -> Self {
        Self {
            generation,
            entry: Some(entry),
            next_free: FREE_END,
        }
    }
}
