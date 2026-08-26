//! Replay and corpus runners for deterministic model sequences.

use std::{io, os::unix::net::UnixStream};

use zio::{
    Interest, Key, Mode, Registration,
    test_support::{MutationOutcome, MutationStep, ScriptedPoll},
};

use crate::{
    ModelSequenceCaseResult, ModelSequenceCheck, ModelSequenceFailure, ModelSequencePhase,
    ModelSequenceReport,
    model_sequence::{Action, CORPUS_SIZE, corpus_seed},
    model_sequence_coverage::Coverage,
    model_sequence_failure::Divergence,
    model_sequence_generate::generate,
    model_sequence_model::ReferenceModel,
    model_sequence_step::execute,
    model_sequence_verify::verify,
};

pub(crate) const STRANGER_KEY: Key = Key::new(0x5a10_ffff_ffff_ffff);
pub(crate) const STRANGER_INTEREST: Interest = Interest::READABLE;
pub(crate) const STRANGER_MODE: Mode = Mode::Level;

pub(crate) struct SequenceContext {
    pub(crate) poll: ScriptedPoll,
    pub(crate) stranger: ScriptedPoll,
    pub(crate) stranger_registration: Registration,
    pub(crate) model: ReferenceModel,
}

/// Replays one bounded generated sequence from its exact seed.
pub fn run_model_sequence(seed: u64) -> Result<(), ModelSequenceFailure> {
    let program = generate(seed).map_err(|()| setup_failure(seed, "action storage"))?;
    run_program(seed, &program.actions)
}

/// Runs the stable 64-seed deterministic corpus.
pub fn run_model_sequences() -> ModelSequenceReport {
    let mut results = Vec::new();
    if results.try_reserve_exact(CORPUS_SIZE).is_err() {
        return ModelSequenceReport::new(
            vec![ModelSequenceCaseResult::failed(setup_failure(
                corpus_seed(0),
                "result storage",
            ))],
            None,
        );
    }
    let mut coverage = Coverage::default();
    for index in 0..CORPUS_SIZE {
        let seed = corpus_seed(index);
        let Ok(program) = generate(seed) else {
            results.push(ModelSequenceCaseResult::failed(setup_failure(
                seed,
                "action storage",
            )));
            continue;
        };
        coverage.merge(&program.coverage);
        results.push(match run_program(seed, &program.actions) {
            Ok(()) => ModelSequenceCaseResult::passed(seed, program.actions.len()),
            Err(failure) => ModelSequenceCaseResult::failed(failure),
        });
    }
    let coverage_failure = (!coverage.is_complete()).then(|| {
        let trace = Vec::new();
        ModelSequenceFailure::new(
            corpus_seed(0),
            ModelSequencePhase::Coverage,
            &trace,
            ModelSequenceCheck::Coverage,
            "all mutation outcomes, disarm, rearm, reuse, stale, and wrong-poller probes",
            coverage.summary(),
        )
    });
    ModelSequenceReport::new(results, coverage_failure)
}

pub(crate) fn run_program(seed: u64, actions: &[Action]) -> Result<(), ModelSequenceFailure> {
    let mut trace = Vec::new();
    trace
        .try_reserve_exact(actions.len())
        .map_err(|_| setup_failure(seed, "replay trace storage"))?;
    let mut context = setup(actions)
        .map_err(|divergence| failure(seed, ModelSequencePhase::Setup, &trace, divergence))?;
    for (step, action) in actions.iter().copied().enumerate() {
        trace.push(action.name());
        let phase = ModelSequencePhase::Action { index: step };
        execute(&mut context, action).map_err(|error| failure(seed, phase, &trace, error))?;
        verify(&context).map_err(|error| failure(seed, phase, &trace, error))?;
    }
    context.poll.finish().map_err(|error| {
        failure(
            seed,
            ModelSequencePhase::Finalize,
            &trace,
            calls("consumed script", error),
        )
    })?;
    context.stranger.finish().map_err(|error| {
        failure(
            seed,
            ModelSequencePhase::Finalize,
            &trace,
            calls("consumed stranger script", error),
        )
    })
}

fn setup(actions: &[Action]) -> Result<SequenceContext, Divergence> {
    let mut steps = Vec::new();
    steps
        .try_reserve_exact(actions.len())
        .map_err(|_| setup_divergence("script storage", "allocation failure"))?;
    steps.extend(actions.iter().filter_map(|action| mutation_step(*action)));
    let poll = ScriptedPoll::with_capacity(1, steps)
        .map_err(|error| setup_divergence("scripted poll", error))?;
    let mut stranger =
        ScriptedPoll::with_capacity(1, [MutationStep::Register(MutationOutcome::Success)])
            .map_err(|error| setup_divergence("stranger poll", error))?;
    let stranger_source = UnixStream::pair()
        .map(|pair| pair.0)
        .map_err(|error| setup_divergence("stranger source", error))?;
    let stranger_registration = stranger
        .register(
            &stranger_source,
            STRANGER_KEY,
            STRANGER_INTEREST,
            STRANGER_MODE,
        )
        .map_err(|error| setup_divergence("stranger registration", error))?;
    let model = ReferenceModel::new()
        .map_err(|()| setup_divergence("reference model storage", "allocation failure"))?;
    Ok(SequenceContext {
        poll,
        stranger,
        stranger_registration,
        model,
    })
}

fn mutation_step(action: Action) -> Option<MutationStep> {
    match action {
        Action::Register { outcome, .. } => Some(MutationStep::Register(
            outcome.mutation(io::ErrorKind::PermissionDenied),
        )),
        Action::Modify { outcome, .. } => Some(MutationStep::Modify(
            outcome.mutation(io::ErrorKind::TimedOut),
        )),
        Action::Delete { outcome } => Some(MutationStep::Delete(
            outcome.mutation(io::ErrorKind::BrokenPipe),
        )),
        Action::RegisterInvalid { .. }
        | Action::Disarm
        | Action::ModifyInvalid { .. }
        | Action::ProbeStale
        | Action::ProbeWrongPoller => None,
    }
}

fn setup_failure(seed: u64, actual: &str) -> ModelSequenceFailure {
    let trace = Vec::new();
    ModelSequenceFailure::new(
        seed,
        ModelSequencePhase::Setup,
        &trace,
        ModelSequenceCheck::Setup,
        "model sequence fixture",
        actual,
    )
}

fn failure(
    seed: u64,
    phase: ModelSequencePhase,
    trace: &[String],
    divergence: Divergence,
) -> ModelSequenceFailure {
    ModelSequenceFailure::new(
        seed,
        phase,
        trace,
        divergence.check,
        divergence.expected,
        divergence.actual,
    )
}

fn setup_divergence(expected: impl std::fmt::Debug, actual: impl std::fmt::Debug) -> Divergence {
    Divergence::new(
        ModelSequenceCheck::Setup,
        format!("{expected:?}"),
        format!("{actual:?}"),
    )
}

fn calls(expected: impl std::fmt::Debug, actual: impl std::fmt::Debug) -> Divergence {
    Divergence::new(
        ModelSequenceCheck::Calls,
        format!("{expected:?}"),
        format!("{actual:?}"),
    )
}
