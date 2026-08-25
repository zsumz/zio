//! Post-observation kqueue recovery reduction.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use crate::{
    ArmState, CommitStatus, Error, Event, Events, Mode, Operation, RecoveryFailure,
    RecoveryOutcome, RegistrationState, pending_kqueue::PendingResource, table::RegistrationTable,
};

#[allow(
    clippy::too_many_arguments,
    reason = "the post-observation boundary keeps each retained contract explicit"
)]
pub(crate) fn finish(
    registrations: &mut RegistrationTable,
    events: &mut Events,
    pending: &[PendingResource],
    delivered: usize,
    woke: bool,
    wake_key: Option<crate::Key>,
    outcomes: &[RecoveryOutcome],
    source: Option<std::io::Error>,
) -> Result<(), Error> {
    validate(
        registrations,
        pending,
        delivered,
        outcomes,
        source.is_some(),
    )?;

    for pending in pending.iter().take(delivered) {
        events
            .try_push(Event::Resource {
                key: pending.key,
                readiness: pending.readiness,
            })
            .map_err(|_| Error::Invariant)?;
    }
    if let (true, Some(key)) = (woke, wake_key) {
        events
            .try_push(Event::Wake { key })
            .map_err(|_| Error::Invariant)?;
    }

    let mut snapshot = source.as_ref().map(|_| Vec::with_capacity(outcomes.len()));
    for outcome in outcomes {
        let registration = outcome.registration();
        let commit = outcome.commit();
        let state = registrations.apply_disarm(registration, commit)?;
        if outcome.state() != state {
            return Err(Error::Invariant);
        }
        if let Some(snapshot) = &mut snapshot {
            snapshot.push(*outcome);
        }
    }
    if let Some(source) = source {
        return Err(Error::Recovery(RecoveryFailure::new(
            Operation::Disarm,
            snapshot.ok_or(Error::Invariant)?,
            source,
        )));
    }
    Ok(())
}

fn validate(
    registrations: &RegistrationTable,
    pending: &[PendingResource],
    delivered: usize,
    outcomes: &[RecoveryOutcome],
    has_source: bool,
) -> Result<(), Error> {
    let delivered = pending.get(..delivered).ok_or(Error::Invariant)?;
    let mut outcome_index = 0;
    for pending in delivered {
        let binding = registrations
            .binding(pending.registration, false)
            .map_err(|_| Error::Invariant)?;
        if binding.mode != Mode::OneShot {
            continue;
        }
        if binding.state
            != (RegistrationState::Registered {
                arm: ArmState::Armed,
            })
        {
            return Err(Error::Invariant);
        }
        let outcome = outcomes.get(outcome_index).ok_or(Error::Invariant)?;
        if outcome.registration() != pending.registration {
            return Err(Error::Invariant);
        }
        outcome_index += 1;
    }
    if outcome_index != outcomes.len() {
        return Err(Error::Invariant);
    }
    let recovery_required = outcomes
        .iter()
        .any(|outcome| outcome.commit() != CommitStatus::Applied);
    if has_source != recovery_required {
        return Err(Error::Invariant);
    }
    Ok(())
}
