//! Generated transition coverage for the deterministic corpus.

#[derive(Clone, Debug, Default)]
pub(crate) struct Coverage {
    pub(crate) register: [bool; 4],
    pub(crate) modify: [bool; 4],
    pub(crate) modify_with_key: [bool; 4],
    pub(crate) delete: [bool; 4],
    pub(crate) special: u16,
}

impl Coverage {
    pub(crate) const DISARM: u16 = 1 << 0;
    pub(crate) const REARM: u16 = 1 << 1;
    pub(crate) const REUSE: u16 = 1 << 2;
    pub(crate) const STALE: u16 = 1 << 3;
    pub(crate) const WRONG_POLLER: u16 = 1 << 4;
    pub(crate) const INVALID_REGISTER: u16 = 1 << 5;
    pub(crate) const INVALID_MODIFY: u16 = 1 << 6;
    pub(crate) const SET_KEY_ARMED: u16 = 1 << 7;
    pub(crate) const SET_KEY_DISARMED: u16 = 1 << 8;
    pub(crate) const SET_KEY_UNCERTAIN: u16 = 1 << 9;
    const ALL_SPECIAL: u16 = (1 << 10) - 1;

    pub(crate) fn merge(&mut self, other: &Self) {
        merge_flags(&mut self.register, other.register);
        merge_flags(&mut self.modify, other.modify);
        merge_flags(&mut self.modify_with_key, other.modify_with_key);
        merge_flags(&mut self.delete, other.delete);
        self.special |= other.special;
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.has_outcome_matrix()
            && self.modify_with_key.iter().all(|covered| *covered)
            && self.special == Self::ALL_SPECIAL
    }

    pub(crate) fn has_outcome_matrix(&self) -> bool {
        self.register.iter().all(|covered| *covered)
            && self.modify.iter().all(|covered| *covered)
            && self.delete.iter().all(|covered| *covered)
    }

    #[cfg(test)]
    pub(crate) const fn has_disarm_rearm(&self) -> bool {
        self.has(Self::DISARM) && self.has(Self::REARM)
    }

    #[cfg(test)]
    pub(crate) const fn has_stale_reuse(&self) -> bool {
        self.has(Self::STALE) && self.has(Self::REUSE)
    }

    #[cfg(test)]
    pub(crate) const fn has_wrong_poller(&self) -> bool {
        self.has(Self::WRONG_POLLER)
    }

    pub(crate) fn summary(&self) -> String {
        format!(
            "register={:?}, modify={:?}, modify_with_key={:?}, delete={:?}, disarm={}, rearm={}, reuse={}, stale={}, wrong_poller={}, invalid_register={}, invalid_modify={}, set_key_armed={}, set_key_disarmed={}, set_key_uncertain={}",
            self.register,
            self.modify,
            self.modify_with_key,
            self.delete,
            self.has(Self::DISARM),
            self.has(Self::REARM),
            self.has(Self::REUSE),
            self.has(Self::STALE),
            self.has(Self::WRONG_POLLER),
            self.has(Self::INVALID_REGISTER),
            self.has(Self::INVALID_MODIFY),
            self.has(Self::SET_KEY_ARMED),
            self.has(Self::SET_KEY_DISARMED),
            self.has(Self::SET_KEY_UNCERTAIN),
        )
    }

    pub(crate) fn mark(&mut self, flag: u16) {
        self.special |= flag;
    }

    const fn has(&self, flag: u16) -> bool {
        self.special & flag != 0
    }
}

fn merge_flags(target: &mut [bool; 4], source: [bool; 4]) {
    for (target, source) in target.iter_mut().zip(source) {
        *target |= source;
    }
}
