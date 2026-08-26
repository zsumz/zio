//! Result, identity, and call assertions for generated mutation actions.

use std::io;

use zio::{
    DeleteError, Error, Interest, Key, Mode, Operation, RegisterError, Registration,
    RegistrationId, test_support::MutationCall,
};

use crate::{
    ModelSequenceCheck, model_sequence::Outcome, model_sequence_failure::Divergence,
    model_sequence_runner::SequenceContext,
};

pub(crate) fn successful_register(
    result: Result<Registration, RegisterError>,
    expected: RegistrationId,
) -> Result<Registration, Divergence> {
    let registration = result.map_err(|error| result_mismatch("successful register", error))?;
    if registration.id() == expected {
        Ok(registration)
    } else {
        Err(handle(expected, registration.id()))
    }
}

pub(crate) fn failed_register(
    result: Result<Registration, RegisterError>,
    outcome: Outcome,
    expected: RegistrationId,
) -> Result<Option<Registration>, Divergence> {
    let Err(error) = result else {
        return Err(result_mismatch("register failure", "success"));
    };
    let borrowed = error.registration().copied();
    let (cause, returned) = error.into_parts();
    expect_mutation_error(
        cause,
        outcome,
        Operation::Register,
        io::ErrorKind::PermissionDenied,
    )?;
    if borrowed != returned {
        return Err(handle(format!("{borrowed:?}"), format!("{returned:?}")));
    }
    match outcome {
        Outcome::NotApplied if returned.is_none() => Ok(None),
        Outcome::Applied | Outcome::Unknown => {
            let registration = returned.ok_or_else(|| handle(expected, "no handle"))?;
            if registration.id() == expected {
                Ok(Some(registration))
            } else {
                Err(handle(expected, registration.id()))
            }
        }
        _ => Err(handle("no handle for not-applied register", returned)),
    }
}

pub(crate) fn expect_mutation_result(
    result: Result<(), Error>,
    outcome: Outcome,
    operation: Operation,
    kind: io::ErrorKind,
) -> Result<(), Divergence> {
    match (outcome, result) {
        (Outcome::Success, Ok(())) => Ok(()),
        (Outcome::Success, Err(error)) => Err(result_mismatch("successful mutation", error)),
        (_, Ok(())) => Err(result_mismatch("mutation failure", "success")),
        (_, Err(error)) => expect_mutation_error(error, outcome, operation, kind),
    }
}

pub(crate) fn expect_delete_error(
    result: Result<(), DeleteError>,
    outcome: Outcome,
    expected: Registration,
) -> Result<(), Divergence> {
    let Err(error) = result else {
        return Err(result_mismatch("delete failure", "success"));
    };
    let borrowed = *error.registration();
    let (cause, returned) = error.into_parts();
    if borrowed != expected || returned != expected {
        return Err(handle(expected, (borrowed, returned)));
    }
    expect_mutation_error(cause, outcome, Operation::Delete, io::ErrorKind::BrokenPipe)
}

pub(crate) fn expect_invalid_interest(error: Error) -> Result<(), Divergence> {
    if matches!(error, Error::InvalidInterest) {
        Ok(())
    } else {
        Err(result_mismatch("InvalidInterest", error))
    }
}

pub(crate) fn expect_call_count(
    expected: usize,
    actual: usize,
    action: &str,
) -> Result<(), Divergence> {
    if actual == expected {
        Ok(())
    } else {
        Err(Divergence::new(
            ModelSequenceCheck::Calls,
            format!("{expected} calls before {action}"),
            format!("{actual} calls after {action}"),
        ))
    }
}

pub(crate) fn last_register_id(
    context: &SequenceContext,
    key: Key,
    interest: Interest,
    mode: Mode,
) -> Result<RegistrationId, Divergence> {
    match context.poll.calls().last().copied() {
        Some(MutationCall::Register {
            registration,
            key: actual_key,
            interest: actual_interest,
            mode: actual_mode,
        }) if (actual_key, actual_interest, actual_mode) == (key, interest, mode) => {
            Ok(registration)
        }
        actual => Err(Divergence::new(
            ModelSequenceCheck::Calls,
            format!("Register({key:?}, {interest:?}, {mode:?})"),
            format!("{actual:?}"),
        )),
    }
}

pub(crate) fn setup(expected: impl std::fmt::Debug, actual: impl std::fmt::Debug) -> Divergence {
    divergence(ModelSequenceCheck::Setup, expected, actual)
}

pub(crate) fn precondition(
    expected: impl std::fmt::Debug,
    actual: impl std::fmt::Debug,
) -> Divergence {
    divergence(ModelSequenceCheck::Precondition, expected, actual)
}

pub(crate) fn result_mismatch(
    expected: impl std::fmt::Debug,
    actual: impl std::fmt::Debug,
) -> Divergence {
    divergence(ModelSequenceCheck::Result, expected, actual)
}

pub(crate) fn handle(expected: impl std::fmt::Debug, actual: impl std::fmt::Debug) -> Divergence {
    divergence(ModelSequenceCheck::Handle, expected, actual)
}

pub(crate) fn generation(
    expected: impl std::fmt::Debug,
    actual: impl std::fmt::Debug,
) -> Divergence {
    divergence(ModelSequenceCheck::Generation, expected, actual)
}

fn expect_mutation_error(
    error: Error,
    outcome: Outcome,
    operation: Operation,
    kind: io::ErrorKind,
) -> Result<(), Divergence> {
    let expected_commit = outcome
        .commit()
        .ok_or_else(|| result_mismatch("failed mutation outcome", "success"))?;
    match error {
        Error::Mutation(mutation)
            if mutation.operation() == operation
                && mutation.commit() == expected_commit
                && mutation.source().kind() == kind =>
        {
            Ok(())
        }
        actual => Err(Divergence::new(
            ModelSequenceCheck::Commit,
            format!("{operation:?}/{expected_commit:?}/{kind:?}"),
            format!("{actual:?}"),
        )),
    }
}

fn divergence(
    check: ModelSequenceCheck,
    expected: impl std::fmt::Debug,
    actual: impl std::fmt::Debug,
) -> Divergence {
    Divergence::new(check, format!("{expected:?}"), format!("{actual:?}"))
}
