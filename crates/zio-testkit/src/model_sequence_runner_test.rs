//! Focused replay-diagnostic tests with access to the private action vocabulary.

use std::io;

use crate::{
    ModelSequenceCheck, ModelSequencePhase,
    model_sequence::{
        ACTION_LIMIT, Action, MODEL_SEQUENCE_DISARM_REARM_SEED, MODEL_SEQUENCE_OUTCOME_MATRIX_SEED,
        MODEL_SEQUENCE_SENTINEL_SEEDS, MODEL_SEQUENCE_STALE_REUSE_SEED,
        MODEL_SEQUENCE_WRONG_POLLER_SEED, Outcome, corpus_seed,
    },
    model_sequence_generate::generate,
    model_sequence_runner::run_program,
};

#[test]
fn failing_sequence_keeps_seed_and_minimal_replay_prefix() -> Result<(), io::Error> {
    let seed = MODEL_SEQUENCE_SENTINEL_SEEDS[0];
    let actions = [Action::Disarm, Action::ProbeStale];
    let first = failure(run_program(seed, &actions))?;
    let second = failure(run_program(seed, &actions))?;

    check_eq(&first, &second, "deterministic failure")?;
    check_eq(&first.seed(), &seed, "replay seed")?;
    check_eq(&first.step(), &Some(0), "first failing step")?;
    check_eq(
        &first.phase(),
        &ModelSequencePhase::Action { index: 0 },
        "failure phase",
    )?;
    check_eq(
        &first.trace(),
        &["delivery.disarm".to_owned()].as_slice(),
        "minimal replay prefix",
    )?;
    check_eq(
        &first.check(),
        &ModelSequenceCheck::Precondition,
        "failure checkpoint",
    )
}

#[test]
fn sentinel_generation_is_replay_stable() -> Result<(), io::Error> {
    for seed in MODEL_SEQUENCE_SENTINEL_SEEDS {
        let first = generate(seed).map_err(|()| io::Error::other("first generation failed"))?;
        let second = generate(seed).map_err(|()| io::Error::other("replay generation failed"))?;
        check_eq(&first.actions, &second.actions, "sentinel action trace")?;
        check_eq(&first.actions.len(), &ACTION_LIMIT, "bounded action count")?;
    }
    Ok(())
}

#[test]
fn curated_sentinels_keep_their_named_behavior() -> Result<(), io::Error> {
    let cases = [
        (
            MODEL_SEQUENCE_OUTCOME_MATRIX_SEED,
            corpus_seed(0),
            "outcome matrix",
        ),
        (
            MODEL_SEQUENCE_DISARM_REARM_SEED,
            corpus_seed(4),
            "disarm and rearm",
        ),
        (
            MODEL_SEQUENCE_STALE_REUSE_SEED,
            corpus_seed(3),
            "stale and reuse",
        ),
        (
            MODEL_SEQUENCE_WRONG_POLLER_SEED,
            corpus_seed(35),
            "wrong poller",
        ),
    ];
    for (actual, expected, context) in cases {
        check_eq(&actual, &expected, context)?;
    }

    let outcome = generate(MODEL_SEQUENCE_OUTCOME_MATRIX_SEED)
        .map_err(|()| io::Error::other("outcome sentinel generation failed"))?;
    check(outcome.coverage.has_outcome_matrix(), "outcome matrix")?;
    let disarm = generate(MODEL_SEQUENCE_DISARM_REARM_SEED)
        .map_err(|()| io::Error::other("disarm sentinel generation failed"))?;
    check(disarm.coverage.has_disarm_rearm(), "disarm and rearm")?;
    let stale = generate(MODEL_SEQUENCE_STALE_REUSE_SEED)
        .map_err(|()| io::Error::other("stale sentinel generation failed"))?;
    check(stale.coverage.has_stale_reuse(), "stale and reuse")?;
    let wrong = generate(MODEL_SEQUENCE_WRONG_POLLER_SEED)
        .map_err(|()| io::Error::other("wrong-poller sentinel generation failed"))?;
    check(wrong.coverage.has_wrong_poller(), "wrong poller")
}

#[test]
fn sentinel_action_fingerprints_are_stable() -> Result<(), io::Error> {
    let expected = [
        0x008f_3825_06df_a353,
        0x5468_3784_1556_cc75,
        0x01d9_3c58_8a70_ef98,
        0x8b9a_02c8_34cf_4280,
    ];
    let mut actual = [0_u64; 4];
    for (index, seed) in MODEL_SEQUENCE_SENTINEL_SEEDS.into_iter().enumerate() {
        let program = generate(seed)
            .map_err(|()| io::Error::other("sentinel fingerprint generation failed"))?;
        check_eq(
            &program.actions.len(),
            &ACTION_LIMIT,
            "bounded action count",
        )?;
        actual[index] = fingerprint(&program.actions);
    }
    check_eq(&actual, &expected, "sentinel action fingerprints")
}

fn fingerprint(actions: &[Action]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for action in actions {
        match *action {
            Action::Register {
                outcome,
                key,
                interest,
                mode,
            } => {
                hash = feed(hash, 0);
                hash = feed(hash, outcome_byte(outcome));
                for byte in key.get().to_le_bytes() {
                    hash = feed(hash, byte);
                }
                hash = feed(hash, interest_byte(interest));
                hash = feed(hash, mode_byte(mode));
            }
            Action::RegisterInvalid { key, mode } => {
                hash = feed(hash, 1);
                for byte in key.get().to_le_bytes() {
                    hash = feed(hash, byte);
                }
                hash = feed(hash, mode_byte(mode));
            }
            Action::Disarm => hash = feed(hash, 2),
            Action::SetKey { key } => {
                hash = feed(hash, 8);
                for byte in key.get().to_le_bytes() {
                    hash = feed(hash, byte);
                }
            }
            Action::Modify {
                outcome,
                interest,
                mode,
            } => {
                hash = feed(hash, 3);
                hash = feed(hash, outcome_byte(outcome));
                hash = feed(hash, interest_byte(interest));
                hash = feed(hash, mode_byte(mode));
            }
            Action::ModifyInvalid { mode } => {
                hash = feed(hash, 4);
                hash = feed(hash, mode_byte(mode));
            }
            Action::Delete { outcome } => {
                hash = feed(hash, 5);
                hash = feed(hash, outcome_byte(outcome));
            }
            Action::ProbeStale => hash = feed(hash, 6),
            Action::ProbeWrongPoller => hash = feed(hash, 7),
        }
    }
    hash
}

const fn feed(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

const fn outcome_byte(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::Success => 0,
        Outcome::NotApplied => 1,
        Outcome::Applied => 2,
        Outcome::Unknown => 3,
    }
}

fn interest_byte(interest: zio::Interest) -> u8 {
    u8::from(interest.is_readable()) | (u8::from(interest.is_writable()) << 1)
}

const fn mode_byte(mode: zio::Mode) -> u8 {
    match mode {
        zio::Mode::Level => 0,
        zio::Mode::OneShot => 1,
    }
}

fn failure(
    result: Result<(), crate::ModelSequenceFailure>,
) -> Result<crate::ModelSequenceFailure, io::Error> {
    match result {
        Ok(()) => Err(io::Error::other("invalid sequence unexpectedly conformed")),
        Err(failure) => Ok(failure),
    }
}

fn check(condition: bool, context: &str) -> Result<(), io::Error> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "missing sentinel behavior: {context}"
        )))
    }
}

fn check_eq<T>(actual: &T, expected: &T, context: &str) -> Result<(), io::Error>
where
    T: std::fmt::Debug + PartialEq,
{
    if actual == expected {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{context}: expected {expected:?}, observed {actual:?}"
        )))
    }
}
