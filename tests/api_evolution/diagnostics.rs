//! Error and recovery contracts.

use zio::{
    CapacityKind, CapacityReason, CommitStatus, DeleteAllError, DeleteError, DeleteOwnedError,
    Error, Key, MutationError, Operation, RecoveryFailure, RecoveryOutcome, RegisterError,
    RegisterOwnedError, Registration, WaitReport,
};

use super::support::*;

#[test]
fn errors_expose_common_diagnostics_without_matching() {
    let _ = Error::operation as fn(&Error) -> Option<Operation>;
    let _ = Error::commit as fn(&Error) -> Option<CommitStatus>;
    let _ = Error::registration_id as fn(&Error) -> Option<zio::RegistrationId>;
    let _ = Error::waker_key_conflict as fn(&Error) -> Option<(Key, Key)>;
    let _ = Error::capacity_limit as fn(&Error) -> Option<usize>;
    let _ = Error::capacity_kind as fn(&Error) -> Option<CapacityKind>;
    let _ = Error::capacity_reason as fn(&Error) -> Option<CapacityReason>;
    let _ = Error::event_capacity_mismatch as fn(&Error) -> Option<(usize, usize)>;
    let _ = Error::io_error as fn(&Error) -> Option<&std::io::Error>;
    let _ = Error::is_wait_interrupted as fn(&Error) -> bool;
}

#[test]
fn capacity_diagnostics_are_open() {
    assert_display::<CapacityKind>();
    assert_display::<CapacityReason>();
    assert_eq!(capacity_kind_class(CapacityKind::Event), "event");
    assert_eq!(
        capacity_kind_class(CapacityKind::Registration),
        "registration"
    );
    assert_eq!(capacity_reason_class(CapacityReason::Zero), "zero");
    assert_eq!(
        capacity_reason_class(CapacityReason::BackendLimit),
        "backend-limit"
    );
    assert_eq!(
        capacity_reason_class(CapacityReason::StorageUnavailable),
        "storage"
    );
}

#[test]
fn recovery_outcomes_return_registration_handles() {
    let _ = RecoveryOutcome::registration as fn(&RecoveryOutcome) -> Registration;
    let _ = RecoveryFailure::len as fn(&RecoveryFailure) -> usize;
    let _ = RecoveryFailure::is_empty as fn(&RecoveryFailure) -> bool;
    assert_slice::<RecoveryFailure, RecoveryOutcome>();
    let _ = assert_recovery_iterator as fn(&RecoveryFailure);
}

#[test]
fn wait_reports_expose_completion() {
    let _ = WaitReport::is_complete as fn(&WaitReport) -> bool;
    let _ = WaitReport::into_result as fn(WaitReport) -> Result<(), RecoveryFailure>;
}

#[test]
fn errors_return_registration_handles() {
    assert_error_ref::<RegisterError>();
    assert_error_ref::<RegisterOwnedError>();
    assert_error_ref::<DeleteError>();
    assert_error_ref::<DeleteOwnedError>();
    assert_error_ref::<DeleteAllError>();
    let _ = RegisterError::registration as fn(&RegisterError) -> Option<Registration>;
    let _ =
        RegisterOwnedError::descriptor as fn(&RegisterOwnedError) -> Option<&std::os::fd::OwnedFd>;
    let _ = RegisterOwnedError::registration as fn(&RegisterOwnedError) -> Option<Registration>;
    let _ = DeleteOwnedError::descriptor as fn(&DeleteOwnedError) -> Option<&std::os::fd::OwnedFd>;
    let _ = DeleteOwnedError::registration as fn(&DeleteOwnedError) -> Option<Registration>;
    let _ = DeleteError::registration as fn(&DeleteError) -> Registration;
    let _ = DeleteAllError::registration as fn(&DeleteAllError) -> Option<Registration>;
    let _ = Error::registration as fn(&Error) -> Option<Registration>;
}

#[test]
fn public_errors_remain_thread_portable() {
    assert_thread_error::<Error>();
    assert_thread_error::<MutationError>();
    assert_thread_error::<RegisterError>();
    assert_thread_error::<RegisterOwnedError>();
    assert_thread_error::<DeleteError>();
    assert_thread_error::<DeleteOwnedError>();
    assert_thread_error::<DeleteAllError>();
    assert_thread_error::<RecoveryFailure>();
}

#[test]
fn mutation_errors_return_every_owned_detail() {
    let _ =
        MutationError::into_parts as fn(MutationError) -> (Operation, CommitStatus, std::io::Error);
}
