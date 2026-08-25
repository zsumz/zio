//! Structural scripted-driver state regressions.

use std::{io, os::fd::AsFd, os::unix::net::UnixStream};

use crate::{
    ArmState, CommitStatus, Interest, Key, Mode, RegistrationId, RegistrationState,
    mutation::{DeleteRequest, ModifyRequest, MutationDriver, RegisterRequest},
};

use super::{
    MutationOutcome, MutationStep, ScriptError, ScriptedBackendState, driver::ScriptedDriver,
};

const ID: RegistrationId = RegistrationId::new(1);
const INTEREST: Interest = Interest::READABLE;
const MODE: Mode = Mode::OneShot;

#[test]
fn descriptor_mismatch_marks_modified_backend_unknown() -> Result<(), Box<dyn std::error::Error>> {
    let (registered, _registered_peer) = UnixStream::pair()?;
    let (other, _other_peer) = UnixStream::pair()?;
    let mut driver = ScriptedDriver::new([
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Modify(MutationOutcome::Success),
    ]);
    register(&mut driver, &registered)?;

    let failure = match driver.modify(ModifyRequest {
        descriptor: other.as_fd(),
        registration: ID,
        previous_interest: INTEREST,
        previous_mode: MODE,
        previous_arm: ArmState::Armed,
        desired_interest: Interest::WRITABLE,
        desired_mode: Mode::Level,
    }) {
        Ok(()) => return Err(io::Error::other("descriptor mismatch succeeded").into()),
        Err(failure) => failure,
    };

    assert_eq!(failure.commit(), CommitStatus::Unknown);
    assert_eq!(driver.state(ID), ScriptedBackendState::Unknown);
    assert_eq!(
        driver.finish(),
        Err(ScriptError::DescriptorChanged { registration: ID })
    );
    Ok(())
}

#[test]
fn descriptor_mismatch_marks_deleted_backend_unknown() -> Result<(), Box<dyn std::error::Error>> {
    let (registered, _registered_peer) = UnixStream::pair()?;
    let (other, _other_peer) = UnixStream::pair()?;
    let mut driver = ScriptedDriver::new([
        MutationStep::Register(MutationOutcome::Success),
        MutationStep::Delete(MutationOutcome::Success),
    ]);
    register(&mut driver, &registered)?;

    let failure = match driver.delete(DeleteRequest {
        descriptor: other.as_fd(),
        registration: ID,
        interest: INTEREST,
        state: RegistrationState::Registered {
            arm: ArmState::Armed,
        },
    }) {
        Ok(()) => return Err(io::Error::other("descriptor mismatch succeeded").into()),
        Err(failure) => failure,
    };

    assert_eq!(failure.commit(), CommitStatus::Unknown);
    assert_eq!(driver.state(ID), ScriptedBackendState::Unknown);
    assert_eq!(
        driver.finish(),
        Err(ScriptError::DescriptorChanged { registration: ID })
    );
    Ok(())
}

#[test]
fn missing_model_registration_is_recorded_as_unknown() -> Result<(), Box<dyn std::error::Error>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut driver = ScriptedDriver::new([MutationStep::Modify(MutationOutcome::Success)]);

    let failure = match driver.modify(ModifyRequest {
        descriptor: source.as_fd(),
        registration: ID,
        previous_interest: INTEREST,
        previous_mode: MODE,
        previous_arm: ArmState::Armed,
        desired_interest: Interest::WRITABLE,
        desired_mode: Mode::Level,
    }) {
        Ok(()) => return Err(io::Error::other("missing registration succeeded").into()),
        Err(failure) => failure,
    };

    assert_eq!(failure.commit(), CommitStatus::Unknown);
    assert_eq!(driver.state(ID), ScriptedBackendState::Unknown);
    assert_eq!(
        driver.finish(),
        Err(ScriptError::UnknownRegistration { registration: ID })
    );
    Ok(())
}

fn register(driver: &mut ScriptedDriver, source: &UnixStream) -> Result<(), io::Error> {
    driver
        .register(RegisterRequest {
            descriptor: source.as_fd(),
            registration: ID,
            key: Key::new(7),
            interest: INTEREST,
            mode: MODE,
        })
        .map_err(crate::sys::MutationFailure::into_source)
}
