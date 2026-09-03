//! Static mutation boundary shared by production and scripted pollers.

mod authority;
mod driver;
mod machine;
mod register;

pub(crate) use authority::{registration_info, registration_state, set_registration_key};
pub(crate) use driver::{DeleteRequest, ModifyRequest, MutationDriver, RegisterRequest};
pub(crate) use machine::MutationSession;
