//! Mutation failure detail regressions.

use std::io;

use crate::{CapacityKind, Registration, RegistrationId};

use super::{CommitStatus, DeleteError, Error, MutationError, Operation, RegisterError};

#[test]
fn mutation_failure_returns_every_owned_part() {
    let failure = MutationError::new(
        Operation::Modify,
        CommitStatus::Unknown,
        io::Error::from_raw_os_error(5),
    );

    let (operation, commit, source) = failure.into_parts();
    assert_eq!(operation, Operation::Modify);
    assert_eq!(commit, CommitStatus::Unknown);
    assert_eq!(source.raw_os_error(), Some(5));
}

#[test]
fn diagnostics_use_plain_display_names() {
    assert_eq!(Operation::RegisterWaker.to_string(), "register waker");
    assert_eq!(CommitStatus::NotApplied.to_string(), "not applied");

    let mutation = MutationError::new(
        Operation::Modify,
        CommitStatus::Unknown,
        io::Error::other("native failure"),
    );
    assert_eq!(
        mutation.to_string(),
        "modify failed with unknown commit status: native failure"
    );
    let wait = Error::Io {
        operation: Operation::Wait,
        source: io::Error::other("native failure"),
    };
    assert_eq!(wait.to_string(), "wait failed: native failure");
    let conflict = Error::WakerAlreadyConfigured {
        existing: crate::Key::new(3),
        requested: crate::Key::new(5),
    };
    assert_eq!(
        conflict.to_string(),
        "poller wake key is 3, not requested key 5"
    );
    assert_eq!(
        Error::WrongPoller {
            registration: Registration::test(7),
        }
        .to_string(),
        "registration 7 belongs to another poller"
    );
    assert_eq!(
        Error::Stale {
            registration: RegistrationId::new(8),
        }
        .to_string(),
        "registration 8 is stale"
    );
    assert_eq!(
        Error::Uncertain {
            registration: RegistrationId::new(9),
        }
        .to_string(),
        "registration 9 has uncertain backend state"
    );
    assert_eq!(
        Error::Capacity {
            kind: CapacityKind::Event,
            limit: 0,
        }
        .to_string(),
        "event capacity 0 is unavailable"
    );
}

#[test]
fn capability_errors_borrow_typed_causes() {
    let register = RegisterError::new(Error::InvalidInterest, None);
    assert!(core::ptr::eq(register.error(), register.as_ref()));

    let delete = DeleteError::new(Error::Invariant, Registration::test(9));
    assert!(core::ptr::eq(delete.error(), delete.as_ref()));
}

#[test]
fn top_level_accessors_expose_embedded_diagnostics() {
    let io = Error::Io {
        operation: Operation::Wait,
        source: io::Error::from_raw_os_error(5),
    };
    assert_eq!(io.operation(), Some(Operation::Wait));
    assert_eq!(io.commit(), None);
    assert_eq!(io.registration_id(), None);
    assert_eq!(io.registration(), None);
    assert_eq!(io.waker_key_conflict(), None);
    assert_eq!(io.capacity_limit(), None);
    assert_eq!(io.event_capacity_mismatch(), None);
    assert_eq!(io.io_error().and_then(io::Error::raw_os_error), Some(5));

    let mutation = Error::Mutation(MutationError::new(
        Operation::Delete,
        CommitStatus::Unknown,
        io::Error::from_raw_os_error(6),
    ));
    assert_eq!(mutation.operation(), Some(Operation::Delete));
    assert_eq!(mutation.commit(), Some(CommitStatus::Unknown));
    assert_eq!(
        mutation.io_error().and_then(io::Error::raw_os_error),
        Some(6)
    );

    assert_eq!(
        Error::UnsupportedPlatform.operation(),
        Some(Operation::UnsupportedPlatform)
    );
    assert!(Error::Invariant.io_error().is_none());
    assert_eq!(Error::Invariant.registration_id(), None);

    let registration = Registration::test(7);
    assert_eq!(
        Error::WrongPoller { registration }.registration_id(),
        Some(registration.id())
    );
    assert_eq!(
        Error::WrongPoller { registration }.registration(),
        Some(registration)
    );
    let id = RegistrationId::new(8);
    assert_eq!(
        Error::Stale { registration: id }.registration_id(),
        Some(id)
    );
    assert_eq!(Error::Stale { registration: id }.registration(), None);
    assert_eq!(
        Error::Uncertain { registration: id }.registration_id(),
        Some(id)
    );
    let capacity = Error::Capacity {
        kind: CapacityKind::Registration,
        limit: 17,
    };
    assert_eq!(capacity.capacity_limit(), Some(17));
    assert_eq!(capacity.capacity_kind(), Some(CapacityKind::Registration));
    assert_eq!(
        Error::WakerAlreadyConfigured {
            existing: crate::Key::new(3),
            requested: crate::Key::new(5),
        }
        .waker_key_conflict(),
        Some((crate::Key::new(3), crate::Key::new(5)))
    );
    assert_eq!(
        Error::EventsTooSmall {
            required: 11,
            actual: 7,
        }
        .event_capacity_mismatch(),
        Some((11, 7))
    );
}
