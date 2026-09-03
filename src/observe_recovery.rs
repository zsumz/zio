//! Post-observation kqueue recovery reduction.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

use core::borrow::Borrow;

use crate::{
    ArmState, CommitStatus, Error, Event, Events, Operation, RecoveryFailure, RecoveryOutcome,
    Registration, RegistrationId, RegistrationState, WaitReport, pending_kqueue::PendingResource,
    registration::PollId, table::RegistrationTable,
};

/// Allocation-free result retained until post-observation state reduction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DisarmOutcome {
    registration: RegistrationId,
    commit: CommitStatus,
}

impl DisarmOutcome {
    pub(crate) const fn new(registration: RegistrationId, commit: CommitStatus) -> Self {
        Self {
            registration,
            commit,
        }
    }

    pub(crate) const fn registration(self) -> RegistrationId {
        self.registration
    }

    pub(crate) const fn commit(self) -> CommitStatus {
        self.commit
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the post-observation boundary keeps each retained contract explicit"
)]
pub(crate) fn finish<I>(
    owner: Option<PollId>,
    registrations: &mut RegistrationTable,
    events: &mut Events,
    pending: &[PendingResource],
    delivered: usize,
    woke: bool,
    wake_key: Option<crate::Key>,
    outcomes: I,
    source: Option<std::io::Error>,
) -> Result<WaitReport, Error>
where
    I: IntoIterator,
    I::IntoIter: Clone + ExactSizeIterator,
    I::Item: Borrow<DisarmOutcome>,
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
                registration: Registration::from_verified(
                    owner.ok_or(Error::Invariant)?,
                    pending.registration,
                ),
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
        let registration =
            Registration::from_verified(owner.ok_or(Error::Invariant)?, registration);
        let public = RecoveryOutcome::new(registration, commit);
        if public.state() != state {
            return Err(Error::Invariant);
        }
        if let Some(snapshot) = &mut snapshot {
            snapshot.push(public);
        }
    }
    if let Some(source) = source {
        return Ok(WaitReport::new(Some(RecoveryFailure::new(
            Operation::Disarm,
            snapshot.ok_or(Error::Invariant)?,
            source,
        ))));
    }
    Ok(WaitReport::new(None))
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
    I::Item: Borrow<DisarmOutcome>,
{
    let delivered = pending.get(..delivered).ok_or(Error::Invariant)?;
    let mut observed_outcomes = outcomes.clone();
    for pending in delivered {
        let binding = registrations
            .binding(pending.registration, false)
            .map_err(|_| Error::Invariant)?;
        if !binding.mode.is_one_shot() {
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
