//! Generated transition coverage for the deterministic corpus.

#[derive(Clone, Debug, Default)]
pub(crate) struct Coverage {
    pub(crate) register: [bool; 4],
    pub(crate) modify: [bool; 4],
    pub(crate) delete: [bool; 4],
    pub(crate) special: u8,
}

impl Coverage {
    pub(crate) const DISARM: u8 = 1 << 0;
    pub(crate) const REARM: u8 = 1 << 1;
    pub(crate) const REUSE: u8 = 1 << 2;
    pub(crate) const STALE: u8 = 1 << 3;
    pub(crate) const WRONG_POLLER: u8 = 1 << 4;
    pub(crate) const INVALID_REGISTER: u8 = 1 << 5;
    pub(crate) const INVALID_MODIFY: u8 = 1 << 6;
    const ALL_SPECIAL: u8 = (1 << 7) - 1;

    pub(crate) fn merge(&mut self, other: &Self) {
        merge_flags(&mut self.register, other.register);
        merge_flags(&mut self.modify, other.modify);
        merge_flags(&mut self.delete, other.delete);
        self.special |= other.special;
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.has_outcome_matrix() && self.special == Self::ALL_SPECIAL
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
            "register={:?}, modify={:?}, delete={:?}, disarm={}, rearm={}, reuse={}, stale={}, wrong_poller={}, invalid_register={}, invalid_modify={}",
            self.register,
            self.modify,
            self.delete,
            self.has(Self::DISARM),
            self.has(Self::REARM),
            self.has(Self::REUSE),
            self.has(Self::STALE),
            self.has(Self::WRONG_POLLER),
            self.has(Self::INVALID_REGISTER),
            self.has(Self::INVALID_MODIFY),
        )
    }

    pub(crate) fn mark(&mut self, flag: u8) {
        self.special |= flag;
    }

    const fn has(&self, flag: u8) -> bool {
        self.special & flag != 0
    }
}

fn merge_flags(target: &mut [bool; 4], source: [bool; 4]) {
    for (target, source) in target.iter_mut().zip(source) {
        *target |= source;
    }
}
