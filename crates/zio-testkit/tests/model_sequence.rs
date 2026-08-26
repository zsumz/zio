//! Public replay and corpus guarantees for the mutation-state model.

use std::io;

use zio_testkit::{
    MODEL_SEQUENCE_SENTINEL_SEEDS, ModelSequenceCaseResult, run_model_sequence, run_model_sequences,
};

const CORPUS_SIZE: usize = 64;
const ACTION_LIMIT: usize = 64;

#[test]
fn model_sequence_corpus_is_complete_and_conformant() -> Result<(), io::Error> {
    let report = run_model_sequences();
    require(
        report.len() == CORPUS_SIZE,
        "corpus must contain 64 stable seeds",
    )?;
    require(
        report.is_coverage_complete(),
        "corpus must cover every required transition class",
    )?;
    for result in report.results() {
        verify_passed_case(result)?;
    }
    report
        .into_result()
        .map_err(|report| io::Error::other(report.to_string()))
}

#[test]
fn model_sequence_sentinel_seeds_replay_exactly() -> Result<(), io::Error> {
    for seed in MODEL_SEQUENCE_SENTINEL_SEEDS {
        let first = run_model_sequence(seed);
        let second = run_model_sequence(seed);
        check_eq(&first, &second, "sentinel replay result")?;
        first.map_err(|failure| io::Error::other(failure.to_string()))?;
    }
    Ok(())
}

#[test]
fn model_sequence_results_replay_by_reported_seed() -> Result<(), io::Error> {
    let report = run_model_sequences();
    for result in report.results() {
        run_model_sequence(result.seed())
            .map_err(|failure| io::Error::other(failure.to_string()))?;
    }
    Ok(())
}

fn verify_passed_case(result: &ModelSequenceCaseResult) -> Result<(), io::Error> {
    require(result.is_passed(), "generated sequence must conform")?;
    require(
        result.steps() == ACTION_LIMIT,
        "generated sequence must execute exactly 64 actions",
    )
}

fn require(condition: bool, message: &str) -> Result<(), io::Error> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message))
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
