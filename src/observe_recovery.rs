//! Post-observation kqueue recovery reduction.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use core::borrow::Borrow;

use crate::{
    ArmState, CommitStatus, Error, Event, Events, Mode, Operation, RecoveryFailure,
    RecoveryOutcome, RegistrationState, pending_kqueue::PendingResource, table::RegistrationTable,
};

#[allow(
    clippy::too_many_arguments,
    reason = "the post-observation boundary keeps each retained contract explicit"
)]
pub(crate) fn finish<I>(
    registrations: &mut RegistrationTable,
    events: &mut Events,
    pending: &[PendingResource],
    delivered: usize,
    woke: bool,
    wake_key: Option<crate::Key>,
    outcomes: I,
    source: Option<std::io::Error>,
) -> Result<(), Error>
where
    I: IntoIterator,
    I::IntoIter: Clone + ExactSizeIterator,
    I::Item: Borrow<RecoveryOutcome>,
{
    let outcomes = outcomes.into_iter();
    validate(
        registrations,
        pending,
        delivered,
        outcomes.clone(),
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
        let outcome = *outcome.borrow();
        let registration = outcome.registration();
        let commit = outcome.commit();
        let state = registrations.apply_disarm(registration, commit)?;
        if outcome.state() != state {
            return Err(Error::Invariant);
        }
        if let Some(snapshot) = &mut snapshot {
            snapshot.push(outcome);
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

fn validate<I>(
    registrations: &RegistrationTable,
    pending: &[PendingResource],
    delivered: usize,
    mut outcomes: I,
    has_source: bool,
) -> Result<(), Error>
where
    I: Clone + ExactSizeIterator,
    I::Item: Borrow<RecoveryOutcome>,
{
    let delivered = pending.get(..delivered).ok_or(Error::Invariant)?;
    let mut observed_outcomes = outcomes.clone();
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
        let outcome = observed_outcomes.next().ok_or(Error::Invariant)?;
        let outcome = outcome.borrow();
        if outcome.registration() != pending.registration {
            return Err(Error::Invariant);
        }
    }
    if observed_outcomes.next().is_some() {
        return Err(Error::Invariant);
    }
    let recovery_required =
        outcomes.any(|outcome| outcome.borrow().commit() != CommitStatus::Applied);
    if has_source != recovery_required {
        return Err(Error::Invariant);
    }
    Ok(())
}
