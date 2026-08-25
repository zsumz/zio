//! Bounded, explicit readiness I/O.

#![deny(unsafe_code)]

mod binding;
mod error;
mod event;
mod interest;
mod mode;
mod observe;
mod pending;
mod pending_kqueue;
mod poll;
mod registration;
mod registration_ops;
mod sys;
mod table;
mod token;
mod wait;

pub use error::{
    CommitStatus, DeleteError, Error, MutationError, Operation, RecoveryFailure, RegisterError,
};
pub use event::{Event, Events, Key, Readiness};
pub use interest::Interest;
pub use mode::Mode;
pub use poll::{DEFAULT_EVENT_CAPACITY, DEFAULT_REGISTRATION_CAPACITY, Poll, Waker};
pub use registration::{ArmState, Registration, RegistrationId, RegistrationState};
pub use wait::Wait;
