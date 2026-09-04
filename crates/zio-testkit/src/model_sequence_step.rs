//! Execution and result checks for generated mutation actions.

use std::io;

use zio::{Interest, Key, Mode, Operation};

use crate::{
    model_sequence::{Action, Outcome},
    model_sequence_expect::{
        expect_call_count, expect_delete_error, expect_invalid_interest, expect_mutation_result,
        failed_register, generation, handle, last_register_id, precondition, result_mismatch,
        setup, successful_register,
    },
    model_sequence_failure::Divergence,
    model_sequence_probe::{probe_stale, probe_wrong_poller},
    model_sequence_runner::SequenceContext,
};

pub(crate) fn execute(context: &mut SequenceContext, action: Action) -> Result<(), Divergence> {
    match action {
        Action::Register {
            outcome,
            key,
            interest,
            mode,
        } => register(context, outcome, key, interest, mode),
        Action::RegisterInvalid { key, mode } => register_invalid(context, key, mode),
        Action::Disarm => disarm(context),
        Action::SetKey { key } => set_key(context, key),
        Action::Modify {
            outcome,
            interest,
            mode,
        } => modify(context, outcome, interest, mode),
        Action::ModifyInvalid { mode } => modify_invalid(context, mode),
        Action::Delete { outcome } => delete(context, outcome),
        Action::ProbeStale => probe_stale(context),
        Action::ProbeWrongPoller => probe_wrong_poller(context),
    }
}

fn register_invalid(context: &mut SequenceContext, key: Key, mode: Mode) -> Result<(), Divergence> {
    if context.model.active().is_some() {
        return Err(precondition("vacant registration slot", "active handle"));
    }
    let source = std::os::unix::net::UnixStream::pair()
        .map(|pair| pair.0)
        .map_err(|error| setup("Unix stream source", error))?;
    let calls = context.poll.calls().len();
    let error = context
        .poll
        .register(&source, key, Interest::EMPTY, mode)
        .err()
        .ok_or_else(|| result_mismatch("InvalidInterest register error", "success"))?;
    let (cause, registration) = error.into_parts();
    if registration.is_some() {
        return Err(handle("no registration capability", registration));
    }
    expect_invalid_interest(cause)?;
    expect_call_count(calls, context.poll.calls().len(), "invalid register")
}

fn register(
    context: &mut SequenceContext,
    outcome: Outcome,
    key: Key,
    interest: Interest,
    mode: Mode,
) -> Result<(), Divergence> {
    if context.model.active().is_some() {
        return Err(precondition("vacant registration slot", "active handle"));
    }
    let source = std::os::unix::net::UnixStream::pair()
        .map(|pair| pair.0)
        .map_err(|error| setup("Unix stream source", error))?;
    let result = context.poll.register(&source, key, interest, mode);
    let id = last_register_id(context, key, interest, mode)?;
    context
        .model
        .record_register(id, key, interest, mode)
        .map_err(|actual| generation("fresh registration generation", actual))?;
    let registration = match outcome {
        Outcome::Success => Some(successful_register(result, id)?),
        Outcome::NotApplied | Outcome::Applied | Outcome::Unknown => {
            failed_register(result, outcome, id)?
        }
    };
    context
        .model
        .complete_register(outcome, registration, key, interest, mode)
        .map_err(|actual| precondition("valid register transition", actual))
}

fn disarm(context: &mut SequenceContext) -> Result<(), Divergence> {
    let entry = context
        .model
        .active()
        .ok_or_else(|| precondition("armed one-shot handle", "vacant slot"))?;
    context
        .model
        .disarm()
        .map_err(|actual| precondition("armed one-shot handle", actual))?;
    context
        .poll
        .establish_disarmed(&entry.registration)
        .map_err(|error| result_mismatch("successful delivered one-shot disarm", error))
}

fn set_key(context: &mut SequenceContext, key: Key) -> Result<(), Divergence> {
    let entry = context
        .model
        .active()
        .ok_or_else(|| precondition("active handle", "vacant slot"))?;
    let calls = context.poll.calls().len();
    context
        .poll
        .set_key(&entry.registration, key)
        .map_err(|error| result_mismatch("successful set_key", error))?;
    context
        .model
        .set_key(key)
        .map_err(|actual| precondition("active handle", actual))?;
    expect_call_count(calls, context.poll.calls().len(), "set_key")
}

fn modify(
    context: &mut SequenceContext,
    outcome: Outcome,
    interest: Interest,
    mode: Mode,
) -> Result<(), Divergence> {
    let entry = context
        .model
        .record_modify(interest, mode)
        .map_err(|actual| precondition("proven registered handle", actual))?;
    let result = context.poll.modify(&entry.registration, interest, mode);
    expect_mutation_result(result, outcome, Operation::Modify, io::ErrorKind::TimedOut)?;
    context
        .model
        .complete_modify(outcome, interest, mode)
        .map_err(|actual| precondition("valid modify transition", actual))
}

fn modify_invalid(context: &mut SequenceContext, mode: Mode) -> Result<(), Divergence> {
    let registration = context
        .model
        .active()
        .map(|entry| entry.registration)
        .ok_or_else(|| precondition("active handle", "vacant slot"))?;
    let calls = context.poll.calls().len();
    let result = context.poll.modify(&registration, Interest::EMPTY, mode);
    match result {
        Err(error) => expect_invalid_interest(error)?,
        Ok(()) => return Err(result_mismatch("InvalidInterest modify error", "success")),
    }
    expect_call_count(calls, context.poll.calls().len(), "invalid modify")
}

fn delete(context: &mut SequenceContext, outcome: Outcome) -> Result<(), Divergence> {
    let entry = context
        .model
        .record_delete()
        .map_err(|actual| precondition("active handle", actual))?;
    let result = context.poll.delete(entry.registration);
    match outcome {
        Outcome::Success => {
            if let Err(error) = result {
                return Err(result_mismatch("successful delete", error));
            }
        }
        Outcome::NotApplied | Outcome::Applied | Outcome::Unknown => {
            expect_delete_error(result, outcome, entry.registration)?;
        }
    }
    context
        .model
        .complete_delete(outcome)
        .map_err(|actual| precondition("valid delete transition", actual))
}
