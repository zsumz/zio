//! Exact descriptor settlement for deletion outcomes.

use std::{
    io,
    num::NonZeroUsize,
    os::fd::{AsFd, AsRawFd},
    os::unix::net::UnixStream,
};

use crate::{
    ArmState, CommitStatus, Error, Interest, Key, Mode, RegistrationState, registration::PollOwner,
    sys::MutationFailure, table::RegistrationTable,
};

use super::{DeleteFailure, MutationDriver, MutationSession};
use crate::mutation::{DeleteRequest, ModifyRequest, RegisterRequest};

type TestResult = Result<(), Box<dyn std::error::Error>>;
type Registered = (
    PollOwner,
    RegistrationTable,
    Driver,
    crate::Registration,
    i32,
);

#[test]
fn successful_delete_releases_the_exact_descriptor() -> TestResult {
    let (mut owner, mut registrations, mut driver, registration, raw) = registered(None)?;

    let Ok(descriptor) = MutationSession::new(&mut owner, &mut registrations, &mut driver)
        .delete_descriptor(registration)
    else {
        return Err(Error::Invariant.into());
    };

    assert_eq!(descriptor.as_raw_fd(), raw);
    assert_eq!(registrations.len(), 0);
    Ok(())
}

#[test]
fn failed_delete_preserves_the_exact_capability() -> TestResult {
    for commit in [
        CommitStatus::NotApplied,
        CommitStatus::Applied,
        CommitStatus::Unknown,
    ] {
        let (mut owner, mut registrations, mut driver, registration, raw) =
            registered(Some(commit))?;

        let result = MutationSession::new(&mut owner, &mut registrations, &mut driver)
            .delete_descriptor(registration);

        match (commit, result) {
            (
                CommitStatus::Applied,
                Err(DeleteFailure::Released {
                    error,
                    registration: returned,
                    descriptor,
                }),
            ) => {
                expect_commit(&error, commit);
                assert_eq!(returned, registration);
                assert_eq!(descriptor.as_raw_fd(), raw);
                assert_eq!(registrations.len(), 0);
            }
            (
                CommitStatus::NotApplied | CommitStatus::Unknown,
                Err(DeleteFailure::Retained {
                    error,
                    registration: returned,
                }),
            ) => {
                expect_commit(&error, commit);
                assert_eq!(returned, registration);
                assert_eq!(
                    registrations
                        .binding(registration.id(), true)?
                        .descriptor
                        .as_raw_fd(),
                    raw
                );
                let expected = match commit {
                    CommitStatus::NotApplied => RegistrationState::Registered {
                        arm: ArmState::Armed,
                    },
                    CommitStatus::Unknown => RegistrationState::Uncertain,
                    CommitStatus::Applied => return Err(Error::Invariant.into()),
                };
                assert_eq!(registrations.state(registration.id())?, expected);
            }
            _ => return Err(Error::Invariant.into()),
        }
    }
    Ok(())
}

fn registered(delete: Option<CommitStatus>) -> Result<Registered, Box<dyn std::error::Error>> {
    let (source, _peer) = UnixStream::pair()?;
    let descriptor = source.as_fd().try_clone_to_owned()?;
    let raw = descriptor.as_raw_fd();
    let mut owner = PollOwner::unassigned();
    let mut registrations = RegistrationTable::new(NonZeroUsize::MIN)?;
    let mut driver = Driver(delete);
    let registration = MutationSession::new(&mut owner, &mut registrations, &mut driver)
        .register_owned(descriptor, Key::new(1), Interest::READABLE, Mode::Level)?;
    Ok((owner, registrations, driver, registration, raw))
}

fn expect_commit(error: &Error, expected: CommitStatus) {
    assert!(matches!(
        error,
        Error::Mutation(mutation) if mutation.commit() == expected
    ));
}

struct Driver(Option<CommitStatus>);

impl MutationDriver for Driver {
    fn register(&mut self, _request: RegisterRequest<'_>) -> Result<(), MutationFailure> {
        Ok(())
    }

    fn modify(&mut self, _request: ModifyRequest<'_>) -> Result<(), MutationFailure> {
        Ok(())
    }

    fn delete(&mut self, _request: DeleteRequest<'_>) -> Result<(), MutationFailure> {
        match self.0 {
            Some(commit) => Err(MutationFailure::new(
                commit,
                io::Error::other("planned failure"),
            )),
            None => Ok(()),
        }
    }
}
