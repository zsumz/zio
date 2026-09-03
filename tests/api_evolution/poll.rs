//! Poller construction, mutation, and inspection contracts.

use zio::{
    DeleteAllError, DeleteOwnedError, DescriptorOwnership, Error, Events, Key, Mode, Poll,
    PollBuilder, RegisterOwnedError, Registration, RegistrationInfo, RegistrationState,
};

use super::support::*;

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
    let _ = Poll::has_native_backend as fn() -> bool;
    let _ = Poll::builder as fn() -> PollBuilder;
    let _ = PollBuilder::new as fn() -> PollBuilder;
    let _ = PollBuilder::event_capacity as fn(PollBuilder, usize) -> PollBuilder;
    let _ = PollBuilder::registration_capacity as fn(PollBuilder, usize) -> PollBuilder;
    let _ = PollBuilder::build as fn(PollBuilder) -> Result<Poll, Error>;
    let _ = PollBuilder::build_with_events as fn(PollBuilder) -> Result<(Poll, Events), Error>;
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
    let _ = Poll::modify_with_key
        as fn(&mut Poll, &Registration, Key, zio::Interest, Mode) -> Result<(), Error>;
}
