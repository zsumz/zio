//! Fail-fast bulk-deletion behavior.

use std::{error::Error as StdError, io, os::unix::net::UnixStream};

use zio::{ArmState, CommitStatus, Error, Interest, Key, Mode, RegistrationState};
use zio_testkit::support::{MutationOutcome, MutationStep, ScriptedPoll};

#[test]
fn delete_all_stops_and_returns_the_failed_registration() -> Result<(), Box<dyn StdError>> {
    for commit in [
        CommitStatus::NotApplied,
        CommitStatus::Applied,
        CommitStatus::Unknown,
    ] {
        verify_failure(commit)?;
    }
    Ok(())
}

fn verify_failure(commit: CommitStatus) -> Result<(), Box<dyn StdError>> {
    let sources = [
        UnixStream::pair()?.0,
        UnixStream::pair()?.0,
        UnixStream::pair()?.0,
    ];
    let mut steps = vec![MutationStep::Register(MutationOutcome::Success); 3];
    steps.push(MutationStep::Delete(MutationOutcome::Success));
    steps.push(MutationStep::Delete(MutationOutcome::Failure {
        commit,
        kind: io::ErrorKind::BrokenPipe,
    }));
    let retry_deletes = if commit == CommitStatus::Applied {
        1
    } else {
        2
    };
    steps.extend((0..retry_deletes).map(|_| MutationStep::Delete(MutationOutcome::Success)));
    let mut poll = ScriptedPoll::with_capacity(sources.len(), steps)?;
    let mut registrations = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        registrations.push(poll.register(
            source,
            Key::new(index as u64),
            Interest::READABLE,
            Mode::Level,
        )?);
    }

    let Err(failure) = poll.delete_all() else {
        return Err(io::Error::other("bulk deletion unexpectedly succeeded").into());
    };
    let returned = failure
        .registration()
        .ok_or_else(|| io::Error::other("failed deletion returned no registration"))?;
    assert!(registrations.contains(&returned));
    assert_eq!(failure.error().commit(), Some(commit));
    assert_eq!(poll.registration_count(), retained_after_failure(commit));
    assert_eq!(poll.calls().len(), 5);

    let (cause, consumed) = failure.into_parts();
    assert_eq!(cause.commit(), Some(commit));
    assert_eq!(consumed, Some(returned));
    match commit {
        CommitStatus::NotApplied => assert_registered(&poll, returned)?,
        CommitStatus::Applied => assert_stale(&poll, returned),
        CommitStatus::Unknown => assert_eq!(
            poll.registration_state(&returned)?,
            RegistrationState::Uncertain,
        ),
    }
    assert_state_counts(&poll, &registrations, expected_states(commit))?;

    poll.delete_all()?;
    assert_eq!(poll.registration_count(), 0);
    for registration in registrations {
        assert_stale(&poll, registration);
    }
    poll.finish()?;
    Ok(())
}

const fn retained_after_failure(commit: CommitStatus) -> usize {
    match commit {
        CommitStatus::Applied => 1,
        CommitStatus::NotApplied | CommitStatus::Unknown => 2,
    }
}

const fn expected_states(commit: CommitStatus) -> [usize; 3] {
    match commit {
        CommitStatus::NotApplied => [1, 2, 0],
        CommitStatus::Applied => [2, 1, 0],
        CommitStatus::Unknown => [1, 1, 1],
    }
}

fn assert_state_counts(
    poll: &ScriptedPoll,
    registrations: &[zio::Registration],
    expected: [usize; 3],
) -> Result<(), Box<dyn StdError>> {
    let mut actual = [0; 3];
    for registration in registrations {
        match poll.registration_state(registration) {
            Err(Error::Stale {
                registration: stale,
            }) if stale == registration.id() => {
                actual[0] += 1;
            }
            Ok(RegistrationState::Registered {
                arm: ArmState::Armed,
            }) => actual[1] += 1,
            Ok(RegistrationState::Uncertain) => actual[2] += 1,
            observed => {
                return Err(io::Error::other(format!(
                    "unexpected registration state: {observed:?}"
                ))
                .into());
            }
        }
    }
    assert_eq!(actual, expected);
    Ok(())
}

fn assert_registered(
    poll: &ScriptedPoll,
    registration: zio::Registration,
) -> Result<(), Box<dyn StdError>> {
    assert_eq!(
        poll.registration_state(&registration)?,
        RegistrationState::Registered {
            arm: ArmState::Armed,
        }
    );
    Ok(())
}

fn assert_stale(poll: &ScriptedPoll, registration: zio::Registration) {
    assert!(matches!(
        poll.registration_state(&registration),
        Err(Error::Stale { registration: stale }) if stale == registration.id()
    ));
}
