//! Bounded, explicit readiness I/O.
//!
//! # Quick start
//!
//! ```no_run
//! use std::{net::TcpListener, time::Duration};
//! use zio::{Event, Interest, Key, Mode, Poll, Wait};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let listener = TcpListener::bind("127.0.0.1:0")?;
//! listener.set_nonblocking(true)?;
//! let mut poll = Poll::new()?;
//! let registration = poll.register(&listener, Key::new(7), Interest::READABLE, Mode::Level)?;
//! let mut events = poll.events()?;
//! let report = poll.wait(&mut events, Wait::For(Duration::from_millis(100)))?;
//! for event in &events {
//!     if let Event::Resource { key, readiness, .. } = event {
//!         println!("{key}: {readiness:?}");
//!     }
//! }
//! report.into_result()?;
//! poll.delete(registration)?;
//! # Ok(())
//! # }
//! ```
#![deny(unsafe_code)]

mod binding;
#[cfg(test)]
mod construction_allocation_test;
mod descriptor;
mod error;
mod event;
mod events;
mod interest;
mod mode;
mod mutation;
mod observe;
#[cfg(test)]
mod observe_allocation_test;
mod observe_recovery;
#[cfg(test)]
mod observe_test;
#[cfg(test)]
mod observe_validation_test;
mod pending;
mod pending_kqueue;
#[cfg(test)]
mod pending_kqueue_test;
mod poll;
mod registration;
mod registration_borrowed;
mod registration_debug;
mod registration_id;
mod registration_ops;
mod registration_state;
mod sys;
mod table;
#[cfg(feature = "unstable-test-support")]
#[doc(hidden)]
pub mod test_support;
mod token;
mod wait;
mod wait_report;
#[cfg(test)]
mod wait_report_test;

pub use error::{
    CapacityKind, CapacityReason, CommitStatus, DeleteAllError, DeleteError, Error, MutationError,
};
pub use error::{Operation, RecoveryFailure, RecoveryOutcome, RegisterError, RegisterOwnedError};
pub use event::{Event, Key, Readiness};
pub use events::Events;
pub use interest::Interest;
pub use mode::Mode;
pub use poll::{DEFAULT_EVENT_CAPACITY, DEFAULT_REGISTRATION_CAPACITY, Poll, Waker};
pub use registration::{ArmState, DescriptorOwnership, RegistrationState};
pub use registration::{Registration, RegistrationId, RegistrationInfo};
pub use wait::Wait;
pub use wait_report::WaitReport;
