//! Stored-configuration rearm behavior.

use std::{io, os::unix::net::UnixStream};

use zio::{ArmState, CommitStatus, Error, Interest, Key, Mode, Operation, RegistrationState};
use zio_testkit::support::{
    MutationCall, MutationOutcome, MutationStep, ScriptedBackendState, ScriptedPoll,
};

#[test]
fn rearm_preserves_one_shot_configuration() -> Result<(), Box<dyn std::error::Error>> {
    let source = UnixStream::pair()?.0;
    let interest = Interest::READABLE | Interest::WRITABLE;
    let mut poll = ScriptedPoll::with_capacity(
        1,
        [
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Modify(MutationOutcome::Success),
            MutationStep::Delete(MutationOutcome::Success),
        ],
    )?;
    let registration = poll.register(&source, Key::new(1), interest, Mode::OneShot)?;
    poll.establish_disarmed(&registration)?;

    poll.rearm(&registration)?;

    assert_eq!(
        poll.calls().get(2),
        Some(&MutationCall::Modify {
            registration: registration.id(),
            previous_interest: interest,
            previous_mode: Mode::OneShot,
            previous_arm: ArmState::Disarmed,
            desired_interest: interest,
            desired_mode: Mode::OneShot,
        })
    );
    assert_eq!(
        poll.registration_state(&registration)?,
        RegistrationState::Registered {
            arm: ArmState::Armed,
        }
    );
    assert_eq!(
        poll.backend_state(registration.id()),
        ScriptedBackendState::Registered {
            interest,
            mode: Mode::OneShot,
            arm: ArmState::Armed,
        }
    );

    let calls = poll.calls().len();
    poll.rearm(&registration)?;
    assert_eq!(poll.calls().len(), calls);
    poll.delete(registration)?;
    poll.finish()?;
    Ok(())
}

#[test]
fn rearm_is_a_noop_for_level_registration() -> Result<(), Box<dyn std::error::Error>> {
    let source = UnixStream::pair()?.0;
    let mut poll = ScriptedPoll::with_capacity(
        1,
        [
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Delete(MutationOutcome::Success),
        ],
    )?;
    let registration = poll.register(&source, Key::new(2), Interest::READABLE, Mode::Level)?;
    let calls = poll.calls().len();

    poll.rearm(&registration)?;

    assert_eq!(poll.calls().len(), calls);
    poll.delete(registration)?;
    poll.finish()?;
    Ok(())
}

#[test]
fn rearm_preserves_every_failure_outcome() -> Result<(), Box<dyn std::error::Error>> {
    for commit in [
        CommitStatus::NotApplied,
        CommitStatus::Applied,
        CommitStatus::Unknown,
    ] {
        verify_failure(commit)?;
    }
    Ok(())
}

fn verify_failure(commit: CommitStatus) -> Result<(), Box<dyn std::error::Error>> {
    let source = UnixStream::pair()?.0;
    let mut poll = ScriptedPoll::with_capacity(
        1,
        [
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Modify(MutationOutcome::Failure {
                commit,
                kind: io::ErrorKind::PermissionDenied,
            }),
            MutationStep::Delete(MutationOutcome::Success),
        ],
    )?;
    let registration = poll.register(&source, Key::new(3), Interest::READABLE, Mode::OneShot)?;
    poll.establish_disarmed(&registration)?;

    let error = poll
        .rearm(&registration)
        .err()
        .ok_or_else(|| io::Error::other("scripted rearm unexpectedly succeeded"))?;

    let Error::Mutation(error) = error else {
        return Err(io::Error::other("rearm returned a non-mutation error").into());
    };
    assert_eq!(error.operation(), Operation::Modify);
    assert_eq!(error.commit(), commit);
    let (state, backend) = expected_failure(commit);
    assert_eq!(poll.registration_state(&registration)?, state);
    assert_eq!(poll.backend_state(registration.id()), backend);
    if commit == CommitStatus::Unknown {
        let calls = poll.calls().len();
        assert!(matches!(
            poll.rearm(&registration),
            Err(Error::Uncertain { registration: id }) if id == registration.id()
        ));
        assert_eq!(poll.calls().len(), calls);
    }
    poll.delete(registration)?;
    poll.finish()?;
    Ok(())
}

const fn expected_failure(commit: CommitStatus) -> (RegistrationState, ScriptedBackendState) {
    match commit {
        CommitStatus::NotApplied => registered(ArmState::Disarmed),
        CommitStatus::Applied => registered(ArmState::Armed),
        CommitStatus::Unknown => (RegistrationState::Uncertain, ScriptedBackendState::Unknown),
    }
}

const fn registered(arm: ArmState) -> (RegistrationState, ScriptedBackendState) {
    (
        RegistrationState::Registered { arm },
        ScriptedBackendState::Registered {
            interest: Interest::READABLE,
            mode: Mode::OneShot,
            arm,
        },
    )
}
