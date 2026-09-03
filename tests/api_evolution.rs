//! Downstream matching contracts for open diagnostics and closed domains.

use zio::{
    ArmState, CommitStatus, DeleteOwnedError, DescriptorOwnership, Error, Mode, Operation,
    RegisterOwnedError, RegistrationState, Wait,
};

#[path = "api_evolution/diagnostics.rs"]
mod diagnostics;
#[path = "api_evolution/events.rs"]
mod events;
#[path = "api_evolution/poll.rs"]
mod poll;
#[path = "api_evolution/support.rs"]
mod support;
#[path = "api_evolution/values.rs"]
mod values;

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
