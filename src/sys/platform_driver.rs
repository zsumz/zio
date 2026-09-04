//! Mutation driver binding for the target-selected backend.

use crate::mutation::{DeleteRequest, ModifyRequest, MutationDriver, RegisterRequest};

use super::{MutationFailure, platform::Backend};

impl MutationDriver for Backend {
    #[inline]
    fn register(&mut self, request: RegisterRequest<'_>) -> Result<(), MutationFailure> {
        let _ = request.key;
        Backend::register(
            self,
            request.descriptor,
            request.registration.get(),
            request.interest,
            request.mode,
        )
    }

    fn modify(&mut self, request: ModifyRequest<'_>) -> Result<(), MutationFailure> {
        Backend::modify(
            self,
            request.descriptor,
            request.registration.get(),
            request.previous_interest,
            request.previous_mode,
            request.previous_arm,
            request.desired_interest,
            request.desired_mode,
        )
    }

    #[inline]
    fn delete(&mut self, request: DeleteRequest<'_>) -> Result<(), MutationFailure> {
        let _ = request.registration;
        Backend::delete(self, request.descriptor, request.interest, request.state)
    }
}
