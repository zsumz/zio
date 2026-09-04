//! Full-state comparison after every generated action.

use zio::{DescriptorOwnership, Error};

use crate::{
    ModelSequenceCheck, model_sequence_failure::Divergence, model_sequence_probe::verify_stranger,
    model_sequence_runner::SequenceContext,
};

pub(crate) fn verify(context: &SequenceContext) -> Result<(), Divergence> {
    verify_calls(context)?;
    verify_active(context)?;
    verify_backend(context)?;
    verify_retired(context)?;
    verify_stranger(context)
}

fn verify_calls(context: &SequenceContext) -> Result<(), Divergence> {
    if context.poll.calls() == context.model.calls() {
        Ok(())
    } else {
        Err(Divergence::new(
            ModelSequenceCheck::Calls,
            format!("{:?}", context.model.calls()),
            format!("{:?}", context.poll.calls()),
        ))
    }
}

fn verify_active(context: &SequenceContext) -> Result<(), Divergence> {
    let Some(entry) = context.model.active() else {
        return Ok(());
    };
    let expected = (
        entry.key,
        entry.interest,
        entry.mode,
        entry.state.portable(),
        DescriptorOwnership::Owned,
    );
    let info = context
        .poll
        .registration_info(&entry.registration)
        .map_err(|error| state(expected, error))?;
    let actual = (
        info.key(),
        info.interest(),
        info.mode(),
        info.state(),
        info.descriptor_ownership(),
    );
    if actual == expected {
        Ok(())
    } else {
        Err(state(expected, actual))
    }
}

fn verify_backend(context: &SequenceContext) -> Result<(), Divergence> {
    for id in context.model.issued() {
        let expected = context.model.expected_backend(*id);
        let actual = context.poll.backend_state(*id);
        if actual != expected {
            return Err(Divergence::new(
                ModelSequenceCheck::Backend,
                format!("{id:?}: {expected:?}"),
                format!("{id:?}: {actual:?}"),
            ));
        }
    }
    Ok(())
}

fn verify_retired(context: &SequenceContext) -> Result<(), Divergence> {
    for registration in context.model.retired() {
        let expected = registration.id();
        match context.poll.registration_state(registration) {
            Err(Error::Stale { registration }) if registration == expected => {}
            actual => {
                return Err(Divergence::new(
                    ModelSequenceCheck::State,
                    format!("Stale({expected:?})"),
                    format!("{actual:?}"),
                ));
            }
        }
    }
    Ok(())
}

fn state(expected: impl std::fmt::Debug, actual: impl std::fmt::Debug) -> Divergence {
    Divergence::new(
        ModelSequenceCheck::State,
        format!("{expected:?}"),
        format!("{actual:?}"),
    )
}
