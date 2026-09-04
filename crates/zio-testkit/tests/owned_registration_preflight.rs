//! Owned-registration preflight capability settlement.

use std::{
    error::Error as StdError,
    io::{self, Read, Write},
    os::fd::{AsRawFd, OwnedFd, RawFd},
    os::unix::net::UnixStream,
};

use zio::{
    ArmState, CapacityKind, CapacityReason, Error, Interest, Key, Mode, RegisterOwnedError,
    RegistrationState,
};
use zio_testkit::support::{MutationOutcome, MutationStep, ScriptedPoll};

const KEY: Key = Key::new(821);
type TestResult = Result<(), Box<dyn StdError>>;

#[test]
fn owned_capacity_failure_returns_exact_descriptor_without_backend_work() -> TestResult {
    let filler = UnixStream::pair()?.0;
    let (source, peer) = UnixStream::pair()?;
    let descriptor: OwnedFd = source.into();
    let raw = descriptor.as_raw_fd();
    let mut poll = ScriptedPoll::with_capacity(
        1,
        [
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Delete(MutationOutcome::Success),
        ],
    )?;
    let retained = poll.register(&filler, Key::new(820), Interest::READABLE, Mode::Level)?;
    let calls = poll.calls().len();

    let Err(error) = poll.register_owned(descriptor, KEY, Interest::WRITABLE, Mode::OneShot) else {
        return Err("owned registration unexpectedly exceeded capacity".into());
    };
    let descriptor = match error {
        RegisterOwnedError::Returned {
            error:
                Error::Capacity {
                    kind: CapacityKind::Registration,
                    limit: 1,
                    reason: CapacityReason::Exhausted,
                    ..
                },
            descriptor,
        } => descriptor,
        actual => {
            return Err(io::Error::other(format!(
                "expected returned capacity failure, observed {actual:?}"
            ))
            .into());
        }
    };

    assert_eq!(poll.calls().len(), calls);
    assert_eq!(poll.registration_count(), 1);
    assert_eq!(
        poll.registration_state(&retained)?,
        RegistrationState::Registered {
            arm: ArmState::Armed,
        }
    );
    prove_descriptor_open(descriptor, peer, raw)?;
    poll.delete(retained)?;
    poll.finish()?;
    Ok(())
}

#[test]
fn owned_invalid_interest_returns_exact_descriptor_without_backend_work() -> TestResult {
    let (source, peer) = UnixStream::pair()?;
    let descriptor: OwnedFd = source.into();
    let raw = descriptor.as_raw_fd();
    let mut poll = ScriptedPoll::new(std::iter::empty::<MutationStep>())?;

    let Err(error) = poll.register_owned(descriptor, KEY, Interest::EMPTY, Mode::Level) else {
        return Err("owned registration accepted empty interest".into());
    };
    let descriptor = match error {
        RegisterOwnedError::Returned {
            error: Error::InvalidInterest,
            descriptor,
        } => descriptor,
        actual => {
            return Err(io::Error::other(format!(
                "expected returned invalid-interest failure, observed {actual:?}"
            ))
            .into());
        }
    };

    assert!(poll.calls().is_empty());
    assert_eq!(poll.registration_count(), 0);
    prove_descriptor_open(descriptor, peer, raw)?;
    poll.finish()?;
    Ok(())
}

fn prove_descriptor_open(
    descriptor: OwnedFd,
    mut peer: UnixStream,
    expected_raw: RawFd,
) -> TestResult {
    assert_eq!(descriptor.as_raw_fd(), expected_raw);
    let mut returned = UnixStream::from(descriptor);
    peer.write_all(b"z")?;
    let mut byte = [0_u8; 1];
    returned.read_exact(&mut byte)?;
    assert_eq!(byte, *b"z");
    Ok(())
}
