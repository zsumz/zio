//! Static mutation boundary shared by production and scripted pollers.

mod authority;
mod driver;
mod machine;
mod register;
mod register_session;

pub(crate) use authority::{
    registration_fd, registration_info, registration_state, registrations, set_registration_key,
};
pub(crate) use driver::{DeleteRequest, ModifyRequest, MutationDriver, RegisterRequest};
pub(crate) use machine::MutationSession;
