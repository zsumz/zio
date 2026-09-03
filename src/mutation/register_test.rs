//! Descriptor settlement regressions for registration failures.

use std::{
    io,
    num::NonZeroUsize,
    os::fd::{AsFd, AsRawFd},
    os::unix::net::UnixStream,
};

use crate::{
    ArmState, CommitStatus, Error, Interest, Key, Mode, RegisterOwnedError, RegistrationState,
    descriptor::Descriptor, registration::PollOwner, sys::MutationFailure,
    table::RegistrationTable,
};

use super::{RegisterFailure, register_descriptor};
use crate::mutation::{
    DeleteRequest, ModifyRequest, MutationDriver, MutationSession, RegisterRequest,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn owned_preflight_failure_releases_the_exact_descriptor() -> TestResult {
    let (source, _peer) = UnixStream::pair()?;
    let descriptor = source.as_fd().try_clone_to_owned()?;
    let raw = descriptor.as_raw_fd();
    let mut owner = PollOwner::unassigned();
    let mut registrations = RegistrationTable::new(NonZeroUsize::MIN)?;
    let mut driver = FailingDriver(CommitStatus::Unknown);

    let result = MutationSession::new(&mut owner, &mut registrations, &mut driver).register_owned(
        descriptor,
        Key::new(1),
        Interest::EMPTY,
        Mode::Level,
    );

    let Err(RegisterOwnedError::Returned { error, descriptor }) = result else {
        return Err(Error::Invariant.into());
    };
    assert!(matches!(error, Error::InvalidInterest));
    assert_eq!(descriptor.as_raw_fd(), raw);
    assert!(owner.current().is_none());
    assert_eq!(registrations.len(), 0);
    Ok(())
}

#[test]
fn not_applied_failure_releases_the_exact_descriptor() -> TestResult {
    let (source, _peer) = UnixStream::pair()?;
    let descriptor = source.as_fd().try_clone_to_owned()?;
    let raw = descriptor.as_raw_fd();
    let mut owner = PollOwner::unassigned();
    let mut registrations = RegistrationTable::new(NonZeroUsize::MIN)?;
    let mut driver = FailingDriver(CommitStatus::NotApplied);

    let result = register_descriptor(
        &mut owner,
        &mut registrations,
        &mut driver,
        Descriptor::owned(descriptor),
        Key::new(1),
        Interest::READABLE,
        Mode::Level,
    );

    let Err(RegisterFailure::Released { error, descriptor }) = result else {
        return Err(Error::Invariant.into());
    };
    assert!(matches!(
        error,
        Error::Mutation(ref mutation) if mutation.commit() == CommitStatus::NotApplied
    ));
    assert_eq!(descriptor.as_raw_fd(), raw);
    assert_eq!(registrations.len(), 0);
    Ok(())
}

#[test]
fn possibly_applied_failures_retain_the_exact_registration() -> TestResult {
    for (commit, state) in [
        (
            CommitStatus::Applied,
            RegistrationState::Registered {
                arm: ArmState::Armed,
            },
        ),
        (CommitStatus::Unknown, RegistrationState::Uncertain),
    ] {
        let (source, _peer) = UnixStream::pair()?;
        let descriptor = source.as_fd().try_clone_to_owned()?;
        let raw = descriptor.as_raw_fd();
        let mut owner = PollOwner::unassigned();
        let mut registrations = RegistrationTable::new(NonZeroUsize::MIN)?;
        let mut driver = FailingDriver(commit);

        let result = register_descriptor(
            &mut owner,
            &mut registrations,
            &mut driver,
            Descriptor::owned(descriptor),
            Key::new(2),
            Interest::READABLE,
            Mode::Level,
        );

        let Err(RegisterFailure::Retained {
            error,
            registration,
        }) = result
        else {
            return Err(Error::Invariant.into());
        };
        assert!(matches!(
            error,
            Error::Mutation(ref mutation) if mutation.commit() == commit
        ));
        assert_eq!(registrations.state(registration.id())?, state);
        assert_eq!(
            registrations
                .binding(registration.id(), true)?
                .descriptor
                .as_raw_fd(),
            raw
        );
    }
    Ok(())
}

struct FailingDriver(CommitStatus);

impl FailingDriver {
    fn failure(&self) -> MutationFailure {
        MutationFailure::new(self.0, io::Error::other("planned failure"))
    }
}

impl MutationDriver for FailingDriver {
    fn register(&mut self, _request: RegisterRequest<'_>) -> Result<(), MutationFailure> {
        Err(self.failure())
    }

    fn modify(&mut self, _request: ModifyRequest<'_>) -> Result<(), MutationFailure> {
        Err(self.failure())
    }

    fn delete(&mut self, _request: DeleteRequest<'_>) -> Result<(), MutationFailure> {
        Err(self.failure())
    }
}
