//! Scripted registration information regressions.

use std::{io, os::unix::net::UnixStream};

use crate::{CommitStatus, Interest, Key, Mode, RegistrationState};

use super::{MutationOutcome, MutationStep, ScriptedPoll};

#[test]
fn uncertain_register_retains_requested_configuration() -> Result<(), Box<dyn std::error::Error>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = ScriptedPoll::new([MutationStep::Register(failure(CommitStatus::Unknown))])?;

    let Err(error) = poll.register(&source, Key::new(3), Interest::READABLE, Mode::OneShot) else {
        return Err(io::Error::other("unknown registration unexpectedly succeeded").into());
    };
    let registration = error
        .registration()
        .ok_or_else(|| io::Error::other("unknown registration lost its handle"))?;
    let info = poll.registration_info(&registration)?;

    assert_eq!(poll.registration_count(), 1);
    assert_eq!(poll.registrations()?, vec![registration]);
    assert!(poll.registration_fd(&registration).is_ok());
    assert_eq!(info.key(), Key::new(3));
    assert_eq!(info.interest(), Interest::READABLE);
    assert_eq!(info.mode(), Mode::OneShot);
    assert_eq!(info.state(), RegistrationState::Uncertain);

    poll.set_key(&registration, Key::new(7))?;
    let updated = poll.registration_info(&registration)?;
    assert_eq!(updated.key(), Key::new(7));
    assert_eq!(updated.state(), RegistrationState::Uncertain);
    Ok(())
}

#[test]
fn uncertain_modify_retains_previous_configuration() -> Result<(), Box<dyn std::error::Error>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = ScriptedPoll::new([
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Modify(failure(CommitStatus::Unknown)),
    ])?;
    let registration = poll.register(&source, Key::new(5), Interest::READABLE, Mode::Level)?;

    if poll
        .modify(&registration, Interest::WRITABLE, Mode::OneShot)
        .is_ok()
    {
        return Err(io::Error::other("unknown modification unexpectedly succeeded").into());
    }
    let info = poll.registration_info(&registration)?;

    assert_eq!(poll.registration_count(), 1);
    assert_eq!(info.key(), Key::new(5));
    assert_eq!(info.interest(), Interest::READABLE);
    assert_eq!(info.mode(), Mode::Level);
    assert_eq!(info.state(), RegistrationState::Uncertain);
    Ok(())
}

#[test]
fn failed_register_count_follows_commit_status() -> Result<(), Box<dyn std::error::Error>> {
    for (commit, retained) in [
        (CommitStatus::NotApplied, 0),
        (CommitStatus::Applied, 1),
        (CommitStatus::Unknown, 1),
    ] {
        let (source, _peer) = UnixStream::pair()?;
        let mut poll = ScriptedPoll::new([MutationStep::Register(failure(commit))])?;

        assert!(
            poll.register(&source, Key::new(1), Interest::READABLE, Mode::Level)
                .is_err()
        );
        assert_eq!(poll.registration_count(), retained);
    }
    Ok(())
}

#[test]
fn failed_delete_count_follows_commit_status() -> Result<(), Box<dyn std::error::Error>> {
    for (commit, retained) in [
        (CommitStatus::NotApplied, 1),
        (CommitStatus::Applied, 0),
        (CommitStatus::Unknown, 1),
    ] {
        let (source, _peer) = UnixStream::pair()?;
        let mut poll = ScriptedPoll::new([
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Delete(failure(commit)),
        ])?;
        let registration = poll.register(&source, Key::new(1), Interest::READABLE, Mode::Level)?;

        assert!(poll.delete(registration).is_err());
        assert_eq!(poll.registration_count(), retained);
    }
    Ok(())
}

const fn failure(commit: CommitStatus) -> MutationOutcome {
    MutationOutcome::Failure {
        commit,
        kind: io::ErrorKind::Other,
    }
}
