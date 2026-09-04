//! Borrowed registrations reject ownership transfer before backend work.

#![allow(
    unsafe_code,
    reason = "the borrowed source remains live through proven registration retirement"
)]

use std::{
    error::Error as StdError,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
};

use zio::{
    ArmState, DeleteOwnedError, DescriptorOwnership, Error, Interest, Key, Mode, RegistrationState,
};
use zio_testkit::support::{MutationOutcome, MutationStep, ScriptedPoll};

type TestResult = Result<(), Box<dyn StdError>>;

#[test]
fn borrowed_delete_owned_returns_handle_without_backend_work() -> TestResult {
    let (mut source, mut peer) = UnixStream::pair()?;
    let mut poll = ScriptedPoll::with_capacity(
        1,
        [
            MutationStep::Register(MutationOutcome::Success),
            MutationStep::Delete(MutationOutcome::Success),
        ],
    )?;

    // SAFETY: `source` remains open and uniquely borrowed through cleanup.
    let registration = unsafe {
        poll.register_borrowed(&source, Key::new(813), Interest::READABLE, Mode::OneShot)?
    };
    let calls = poll.calls().len();
    let Err(error) = poll.delete_owned(registration) else {
        return Err(io::Error::other("borrowed deletion returned a descriptor").into());
    };
    let returned = retained_handle(error, registration.id())?;

    assert_eq!(returned, registration);
    assert_eq!(poll.calls().len(), calls);
    let info = poll.registration_info(&returned)?;
    assert_eq!(info.descriptor_ownership(), DescriptorOwnership::Borrowed);
    assert_eq!(
        info.state(),
        RegistrationState::Registered {
            arm: ArmState::Armed,
        }
    );
    prove_source_open(&mut source, &mut peer)?;

    poll.delete(returned)?;
    prove_source_open(&mut source, &mut peer)?;
    poll.finish()?;
    Ok(())
}

fn retained_handle(
    error: DeleteOwnedError,
    expected: zio::RegistrationId,
) -> Result<zio::Registration, io::Error> {
    match error {
        DeleteOwnedError::Retained {
            error: Error::DescriptorNotOwned { registration },
            registration: returned,
        } if registration == expected && returned.id() == expected => Ok(returned),
        actual => Err(io::Error::other(format!(
            "expected retained DescriptorNotOwned handle, observed {actual:?}"
        ))),
    }
}

fn prove_source_open(source: &mut UnixStream, peer: &mut UnixStream) -> TestResult {
    peer.write_all(b"z")?;
    let mut byte = [0_u8; 1];
    source.read_exact(&mut byte)?;
    assert_eq!(byte, *b"z");
    Ok(())
}
