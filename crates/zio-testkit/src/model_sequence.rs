//! Stable action vocabulary for bounded mutation-model sequences.

use std::io;

use zio::{CommitStatus, Interest, Key, Mode, test_support::MutationOutcome};

pub(crate) const CORPUS_SIZE: usize = 64;
pub(crate) const ACTION_LIMIT: usize = 64;
const BASE_SEED: u64 = 0x5a10_5eed_0000_0001;
pub(crate) const SEED_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// Curated seed covering every register, modify, and delete outcome.
pub const MODEL_SEQUENCE_OUTCOME_MATRIX_SEED: u64 = 0x93f8_3818_b027_780d;
/// Curated seed covering delivered one-shot disarm and explicit rearm.
pub const MODEL_SEQUENCE_DISARM_REARM_SEED: u64 = 0xffb3_6b94_4ca5_3a48;
/// Curated seed covering terminal generations, replacement, and stale probes.
pub const MODEL_SEQUENCE_STALE_REUSE_SEED: u64 = 0x2536_9410_a954_fafc;
/// Curated seed covering foreign-poller rejection without backend work.
pub const MODEL_SEQUENCE_WRONG_POLLER_SEED: u64 = 0x5da8_3ce4_73ec_3f07;

/// Stable curated seeds for focused replay in downstream tests.
pub const MODEL_SEQUENCE_SENTINEL_SEEDS: [u64; 4] = [
    MODEL_SEQUENCE_OUTCOME_MATRIX_SEED,
    MODEL_SEQUENCE_DISARM_REARM_SEED,
    MODEL_SEQUENCE_STALE_REUSE_SEED,
    MODEL_SEQUENCE_WRONG_POLLER_SEED,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    Success,
    NotApplied,
    Applied,
    Unknown,
}

impl Outcome {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::NotApplied => "not_applied",
            Self::Applied => "applied",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) const fn commit(self) -> Option<CommitStatus> {
        match self {
            Self::Success => None,
            Self::NotApplied => Some(CommitStatus::NotApplied),
            Self::Applied => Some(CommitStatus::Applied),
            Self::Unknown => Some(CommitStatus::Unknown),
        }
    }

    pub(crate) const fn mutation(self, kind: io::ErrorKind) -> MutationOutcome {
        match self.commit() {
            None => MutationOutcome::Success,
            Some(commit) => MutationOutcome::Failure { commit, kind },
        }
    }

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Success => 0,
            Self::NotApplied => 1,
            Self::Applied => 2,
            Self::Unknown => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Register {
        outcome: Outcome,
        key: Key,
        interest: Interest,
        mode: Mode,
    },
    RegisterInvalid {
        key: Key,
        mode: Mode,
    },
    Disarm,
    Modify {
        outcome: Outcome,
        interest: Interest,
        mode: Mode,
    },
    ModifyInvalid {
        mode: Mode,
    },
    Delete {
        outcome: Outcome,
    },
    ProbeStale,
    ProbeWrongPoller,
}

impl Action {
    pub(crate) fn name(self) -> String {
        match self {
            Self::Register {
                outcome,
                key,
                interest,
                mode,
            } => format!(
                "register.{}.{}.{}.key_{:016x}",
                outcome.name(),
                interest_name(interest),
                mode_name(mode),
                key.get(),
            ),
            Self::RegisterInvalid { key, mode } => format!(
                "register.invalid_interest.{}.key_{:016x}",
                mode_name(mode),
                key.get(),
            ),
            Self::Disarm => "delivery.disarm".to_owned(),
            Self::Modify {
                outcome,
                interest,
                mode,
            } => format!(
                "modify.{}.{}.{}",
                outcome.name(),
                interest_name(interest),
                mode_name(mode)
            ),
            Self::ModifyInvalid { mode } => {
                format!("modify.invalid_interest.{}", mode_name(mode))
            }
            Self::Delete { outcome } => format!("delete.{}", outcome.name()),
            Self::ProbeStale => "probe.stale".to_owned(),
            Self::ProbeWrongPoller => "probe.wrong_poller".to_owned(),
        }
    }
}

pub(crate) const fn corpus_seed(index: usize) -> u64 {
    scramble(
        BASE_SEED
            .wrapping_add(index as u64)
            .wrapping_add(SEED_GAMMA),
    )
}

pub(crate) const fn scramble(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn interest_name(interest: Interest) -> &'static str {
    match (interest.is_readable(), interest.is_writable()) {
        (true, true) => "read_write",
        (true, false) => "readable",
        (false, true) => "writable",
        (false, false) => "empty",
    }
}

const fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Level => "level",
        Mode::OneShot => "one_shot",
    }
}
