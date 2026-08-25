//! Safe request vocabulary for statically dispatched selector mutations.

use std::os::fd::BorrowedFd;

use crate::sys::MutationFailure;
use crate::{ArmState, Interest, Key, Mode, RegistrationId, RegistrationState};

/// One exact native registration request.
#[derive(Debug)]
pub(crate) struct RegisterRequest<'descriptor> {
    pub(crate) descriptor: BorrowedFd<'descriptor>,
    pub(crate) registration: RegistrationId,
    pub(crate) key: Key,
    pub(crate) interest: Interest,
    pub(crate) mode: Mode,
}

/// One exact native modification request with complete prior state.
#[derive(Debug)]
pub(crate) struct ModifyRequest<'descriptor> {
    pub(crate) descriptor: BorrowedFd<'descriptor>,
    pub(crate) registration: RegistrationId,
    pub(crate) previous_interest: Interest,
    pub(crate) previous_mode: Mode,
    pub(crate) previous_arm: ArmState,
    pub(crate) desired_interest: Interest,
    pub(crate) desired_mode: Mode,
}

/// One exact native deletion request.
#[derive(Debug)]
pub(crate) struct DeleteRequest<'descriptor> {
    pub(crate) descriptor: BorrowedFd<'descriptor>,
    pub(crate) registration: RegistrationId,
    pub(crate) interest: Interest,
    pub(crate) state: RegistrationState,
}

/// Statically selected mutation mechanism used by the portable state machine.
pub(crate) trait MutationDriver {
    fn register(&mut self, request: RegisterRequest<'_>) -> Result<(), MutationFailure>;

    fn modify(&mut self, request: ModifyRequest<'_>) -> Result<(), MutationFailure>;

    fn delete(&mut self, request: DeleteRequest<'_>) -> Result<(), MutationFailure>;
}
