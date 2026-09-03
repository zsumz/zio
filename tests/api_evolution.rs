//! Downstream matching contracts for open diagnostics and closed domains.

use zio::{
    ArmState, CapacityKind, CapacityReason, CommitStatus, DeleteAllError, DeleteError,
    DeleteOwnedError, DescriptorOwnership, Error, Event, Events, Key, Mode, MutationError,
    Operation, Poll, PollBuilder, Readiness, RecoveryFailure, RecoveryOutcome, RegisterError,
    RegisterOwnedError, Registration, RegistrationId, RegistrationInfo, RegistrationState, Wait,
    WaitReport, Waker,
};

#[path = "api_evolution/support.rs"]
mod support;
use support::*;

#[test]
fn open_diagnostics_support_forward_compatible_fallbacks() {
    assert_display::<Operation>();
    assert_display::<CommitStatus>();
    assert_eq!(operation_class(Operation::Wait), "wait");
    assert_eq!(operation_class(Operation::Delete), "other");
    assert_eq!(error_class(&Error::Invariant), "contract");
    assert_eq!(error_class(&Error::UnsupportedPlatform), "other");
}

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
fn closed_delivery_and_state_domains_remain_exhaustive() {
    assert_eq!(mode_class(Mode::Level), "level");
    let _ = Mode::is_one_shot as fn(Mode) -> bool;
    assert_eq!(wait_class(Wait::NoBlock), "no-block");
    assert_eq!(commit_class(CommitStatus::Unknown), "unknown");
    assert_eq!(arm_class(ArmState::Disarmed), "disarmed");
    assert_eq!(state_class(RegistrationState::Uncertain), "uncertain");
    assert_eq!(ownership_class(DescriptorOwnership::Borrowed), "borrowed");
    let _ = owned_register_error_class as fn(&RegisterOwnedError) -> &'static str;
    let _ = owned_delete_error_class as fn(&DeleteOwnedError) -> &'static str;
    let _ = Wait::is_nonblocking as fn(Wait) -> bool;
    let _ = RegistrationState::is_registered as fn(RegistrationState) -> bool;
    let _ = RegistrationState::is_uncertain as fn(RegistrationState) -> bool;
    let _ = RegistrationState::arm as fn(RegistrationState) -> Option<ArmState>;
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
fn public_values_remain_thread_portable() {
    assert_copy_thread_value::<Key>();
    assert_copy_thread_value::<zio::Interest>();
    assert_copy_thread_value::<Readiness>();
    assert_copy_thread_value::<Event>();
    assert_copy_thread_value::<Mode>();
    assert_copy_thread_value::<Wait>();
    assert_copy_thread_value::<Registration>();
    assert_copy_thread_value::<RegistrationId>();
    assert_copy_thread_value::<RegistrationInfo>();
    assert_copy_thread_value::<RecoveryOutcome>();
    assert_copy_thread_value::<ArmState>();
    assert_copy_thread_value::<RegistrationState>();
    assert_copy_thread_value::<DescriptorOwnership>();
    assert_copy_thread_value::<Operation>();
    assert_copy_thread_value::<CommitStatus>();
    assert_copy_thread_value::<CapacityKind>();
    assert_copy_thread_value::<CapacityReason>();
    assert_copy_thread_value::<PollBuilder>();
    assert_thread_value::<Events>();
    assert_thread_value::<WaitReport>();
}

#[test]
fn registration_handles_support_ordered_collections() {
    assert_ordered::<Registration>();
}

#[test]
fn registration_ids_support_diagnostic_interop() {
    let _ = RegistrationId::get as fn(RegistrationId) -> u64;
    assert_from::<u64, RegistrationId>();
    assert_display::<RegistrationId>();
}

#[test]
fn wakers_expose_keys_and_cross_thread_traits() {
    let _ = Waker::key as fn(&Waker) -> Key;
    let _ = Poll::waker_key as fn(&Poll) -> Option<Key>;
    assert_send::<Poll>();
    assert_send_sync::<Waker>();
}

#[test]
fn mutation_errors_return_every_owned_detail() {
    let _ =
        MutationError::into_parts as fn(MutationError) -> (Operation, CommitStatus, std::io::Error);
}

#[test]
fn poll_exposes_stored_configuration_rearm() {
    let _ = Poll::rearm as fn(&mut Poll, &Registration) -> Result<(), Error>;
}

#[test]
fn poll_supports_fail_fast_bulk_deletion() {
    let _ = Poll::delete_all as fn(&mut Poll) -> Result<(), DeleteAllError>;
}

#[test]
fn poll_returns_owned_descriptors_on_deletion() {
    let _ = Poll::delete_owned
        as fn(&mut Poll, Registration) -> Result<std::os::fd::OwnedFd, DeleteOwnedError>;
}

#[test]
fn poll_accepts_owned_descriptors_without_duplication() {
    let _ = Poll::register_owned
        as fn(
            &mut Poll,
            std::os::fd::OwnedFd,
            Key,
            zio::Interest,
            Mode,
        ) -> Result<Registration, RegisterOwnedError>;
}

#[test]
fn poll_borrows_retained_registration_descriptors() {
    let _ = Poll::registration_fd
        as for<'poll, 'registration> fn(
            &'poll Poll,
            &'registration Registration,
        ) -> Result<std::os::fd::BorrowedFd<'poll>, Error>;
}

#[test]
fn poll_snapshots_retained_registration_handles() {
    let _ = Poll::registrations as fn(&Poll) -> Result<Vec<Registration>, Error>;
    let _ = assert_registration_iterator as fn(&Poll) -> Result<(), Error>;
}

#[test]
fn poll_exposes_capacity_and_retained_count() {
    let _ = Poll::builder as fn() -> PollBuilder;
    let _ = PollBuilder::new as fn() -> PollBuilder;
    let _ = PollBuilder::event_capacity as fn(PollBuilder, usize) -> PollBuilder;
    let _ = PollBuilder::registration_capacity as fn(PollBuilder, usize) -> PollBuilder;
    let _ = PollBuilder::build as fn(PollBuilder) -> Result<Poll, Error>;
    let _ = Poll::event_capacity as fn(&Poll) -> usize;
    let _ = Poll::registration_capacity as fn(&Poll) -> usize;
    let _ = Poll::registration_count as fn(&Poll) -> usize;
    let _ = Poll::remaining_registration_capacity as fn(&Poll) -> usize;
    let _ = Poll::contains as fn(&Poll, &Registration) -> bool;
    let _ = Poll::is_empty as fn(&Poll) -> bool;
    let _ = Poll::is_full as fn(&Poll) -> bool;
}

#[test]
fn poll_exposes_authoritative_registration_info() {
    let _ = Poll::registration_info as fn(&Poll, &Registration) -> Result<RegistrationInfo, Error>;
    let _ = RegistrationInfo::key as fn(&RegistrationInfo) -> Key;
    let _ = RegistrationInfo::interest as fn(&RegistrationInfo) -> zio::Interest;
    let _ = RegistrationInfo::mode as fn(&RegistrationInfo) -> Mode;
    let _ = RegistrationInfo::state as fn(&RegistrationInfo) -> RegistrationState;
    let _ = RegistrationInfo::descriptor_ownership as fn(&RegistrationInfo) -> DescriptorOwnership;
    let _ = Poll::set_key as fn(&mut Poll, &Registration, Key) -> Result<(), Error>;
}

#[test]
fn flag_sets_support_standard_set_operators() {
    let _ =
        zio::Interest::symmetric_difference as fn(zio::Interest, zio::Interest) -> zio::Interest;
    let _ = Readiness::symmetric_difference as fn(Readiness, Readiness) -> Readiness;
    let _ = zio::Interest::complement as fn(zio::Interest) -> zio::Interest;
    let _ = Readiness::complement as fn(Readiness) -> Readiness;
    assert_eq!(
        zio::Interest::ALL,
        zio::Interest::READABLE | zio::Interest::WRITABLE
    );
    assert!(Readiness::ALL.contains(Readiness::ERROR));
    assert_set_operators::<zio::Interest>();
    assert_set_operators::<Readiness>();
}

#[test]
fn event_and_wait_values_are_hashable() {
    assert_hash::<Event>();
    assert_hash::<Wait>();
    assert_from::<Wait, core::time::Duration>();
    assert_from::<Wait, Option<core::time::Duration>>();
}

#[test]
fn event_batches_support_standard_iteration() {
    assert_slice::<Events, Event>();
    let _ = Events::remaining_capacity as fn(&Events) -> usize;
    let _ = Events::is_full as fn(&Events) -> bool;
    let _ = assert_event_iterators as fn(&mut Events);
    let _ = assert_owned_event_iterator as fn(Events);
}

#[test]
fn events_expose_direct_classification_and_readiness() {
    let _ = Event::is_resource as fn(Event) -> bool;
    let _ = Event::is_wake as fn(Event) -> bool;
    let _ = Event::is_readable as fn(Event) -> bool;
    let _ = Event::is_writable as fn(Event) -> bool;
    let _ = Event::is_read_closed as fn(Event) -> bool;
    let _ = Event::is_write_closed as fn(Event) -> bool;
    let _ = Event::is_error as fn(Event) -> bool;
}

#[test]
fn keys_support_lossless_standard_conversions() {
    assert_from::<Key, u64>();
    assert_from::<u64, Key>();
    assert_display::<Key>();
}
