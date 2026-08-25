//! Static mutation boundary shared by production and scripted pollers.

mod driver;
mod machine;

pub(crate) use driver::{DeleteRequest, ModifyRequest, MutationDriver, RegisterRequest};
pub(crate) use machine::{MutationSession, registration_state};
