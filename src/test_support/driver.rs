//! Scripted stateful implementation of the private mutation driver.

use std::{collections::VecDeque, io, os::fd::AsRawFd};

use crate::{
    CommitStatus, Operation, RegistrationId,
    mutation::{DeleteRequest, ModifyRequest, MutationDriver, RegisterRequest},
    sys::MutationFailure,
};

use super::{
    MutationCall, MutationOutcome, MutationStep, ScriptError, ScriptedBackendState,
    model::BackendModel,
};

/// Stateful scripted driver kept behind the test-support feature.
#[derive(Debug)]
pub(super) struct ScriptedDriver {
    steps: VecDeque<MutationStep>,
    calls: Vec<MutationCall>,
    model: BackendModel,
    fault: Option<ScriptError>,
}

impl ScriptedDriver {
    pub(super) fn new(steps: impl IntoIterator<Item = MutationStep>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            calls: Vec::new(),
            model: BackendModel::default(),
            fault: None,
        }
    }

    pub(super) fn calls(&self) -> &[MutationCall] {
        &self.calls
    }

    pub(super) fn state(&self, registration: RegistrationId) -> ScriptedBackendState {
        self.model.state(registration)
    }

    pub(super) fn establish_disarmed(
        &mut self,
        registration: RegistrationId,
        descriptor: i32,
    ) -> Result<(), ScriptError> {
        if let Err(error) = self.model.establish_disarmed(registration, descriptor) {
            self.fault.get_or_insert(error);
            return Err(error);
        }
        self.calls
            .push(MutationCall::EstablishDisarmed { registration });
        Ok(())
    }

    pub(super) fn finish(&self) -> Result<(), ScriptError> {
        if let Some(fault) = self.fault {
            return Err(fault);
        }
        if self.steps.is_empty() {
            Ok(())
        } else {
            Err(ScriptError::Remaining {
                count: self.steps.len(),
            })
        }
    }

    fn next(&mut self, actual: Operation) -> Result<MutationOutcome, MutationFailure> {
        let Some(step) = self.steps.pop_front() else {
            return Err(self.structural_failure(ScriptError::Exhausted { operation: actual }));
        };
        let expected = step.operation();
        if expected != actual {
            return Err(self.structural_failure(ScriptError::Mismatch { expected, actual }));
        }
        Ok(step.outcome())
    }

    fn structural_failure(&mut self, error: ScriptError) -> MutationFailure {
        self.fault.get_or_insert(error);
        MutationFailure::new(
            CommitStatus::Unknown,
            io::Error::new(io::ErrorKind::InvalidData, error.to_string()),
        )
    }

    fn outcome_result(outcome: MutationOutcome) -> Result<(), MutationFailure> {
        match outcome {
            MutationOutcome::Success => Ok(()),
            MutationOutcome::Failure { commit, kind } => Err(MutationFailure::new(
                commit,
                io::Error::new(kind, "scripted mutation failure"),
            )),
        }
    }
}

impl MutationDriver for ScriptedDriver {
    fn register(&mut self, request: RegisterRequest<'_>) -> Result<(), MutationFailure> {
        self.calls.push(MutationCall::Register {
            registration: request.registration,
            key: request.key,
            interest: request.interest,
            mode: request.mode,
        });
        let descriptor = request.descriptor.as_raw_fd();
        self.model.begin_register(request.registration, descriptor);
        let outcome = match self.next(Operation::Register) {
            Ok(outcome) => outcome,
            Err(failure) => {
                self.model.mark_unknown(request.registration, descriptor);
                return Err(failure);
            }
        };
        self.model.complete_register(
            request.registration,
            request.interest,
            request.mode,
            outcome,
        );
        Self::outcome_result(outcome)
    }

    fn modify(&mut self, request: ModifyRequest<'_>) -> Result<(), MutationFailure> {
        self.calls.push(MutationCall::Modify {
            registration: request.registration,
            previous_interest: request.previous_interest,
            previous_mode: request.previous_mode,
            previous_arm: request.previous_arm,
            desired_interest: request.desired_interest,
            desired_mode: request.desired_mode,
        });
        let descriptor = request.descriptor.as_raw_fd();
        if let Err(error) = self.model.validate(request.registration, descriptor) {
            self.model.mark_unknown(request.registration, descriptor);
            return Err(self.structural_failure(error));
        }
        let outcome = match self.next(Operation::Modify) {
            Ok(outcome) => outcome,
            Err(failure) => {
                self.model.mark_unknown(request.registration, descriptor);
                return Err(failure);
            }
        };
        self.model.complete_modify(
            request.registration,
            request.desired_interest,
            request.desired_mode,
            outcome,
        );
        Self::outcome_result(outcome)
    }

    fn delete(&mut self, request: DeleteRequest<'_>) -> Result<(), MutationFailure> {
        self.calls.push(MutationCall::Delete {
            registration: request.registration,
            interest: request.interest,
            state: request.state,
        });
        let descriptor = request.descriptor.as_raw_fd();
        if let Err(error) = self.model.validate(request.registration, descriptor) {
            self.model.mark_unknown(request.registration, descriptor);
            return Err(self.structural_failure(error));
        }
        let outcome = match self.next(Operation::Delete) {
            Ok(outcome) => outcome,
            Err(failure) => {
                self.model.mark_unknown(request.registration, descriptor);
                return Err(failure);
            }
        };
        self.model.complete_delete(request.registration, outcome);
        Self::outcome_result(outcome)
    }
}
