//! Fail-fast bulk-deletion behavior.

use std::{error::Error as StdError, io, os::unix::net::UnixStream};

use zio::{CommitStatus, Error, Interest, Key, Mode};
use zio_testkit::support::{MutationOutcome, MutationStep, ScriptedPoll};

#[test]
fn delete_all_stops_and_returns_the_failed_registration() -> Result<(), Box<dyn StdError>> {
    let sources = [
        UnixStream::pair()?.0,
        UnixStream::pair()?.0,
        UnixStream::pair()?.0,
    ];
    let mut poll = ScriptedPoll::with_capacity(
        sources.len(),
        [
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Delete(MutationOutcome::Success),
            MutationStep::Delete(MutationOutcome::Failure {
                commit: CommitStatus::NotApplied,
                kind: io::ErrorKind::BrokenPipe,
            }),
            MutationStep::Delete(MutationOutcome::Success),
            MutationStep::Delete(MutationOutcome::Success),
        ],
    )?;
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
    assert_eq!(failure.error().commit(), Some(CommitStatus::NotApplied));
    assert_eq!(poll.registration_count(), 2);
    assert!(poll.registrations()?.contains(&returned));
    assert_eq!(poll.calls().len(), 5);

    let (cause, consumed) = failure.into_parts();
    assert_eq!(cause.commit(), Some(CommitStatus::NotApplied));
    assert_eq!(consumed, Some(returned));
    poll.delete_all()?;
    assert_eq!(poll.registration_count(), 0);
    for registration in registrations {
        assert!(matches!(
            poll.registration_state(&registration),
            Err(Error::Stale { registration: stale }) if stale == registration.id()
        ));
    }
    poll.finish()?;
    Ok(())
}
