//! Descriptor-private backend state used by the scripted mutation driver.

use crate::{ArmState, CommitStatus, Interest, Mode, RegistrationId};

use super::{MutationOutcome, ScriptError, ScriptedBackendState};

#[derive(Clone, Copy, Debug)]
struct BackendEntry {
    registration: RegistrationId,
    descriptor: i32,
    state: ScriptedBackendState,
}

#[derive(Debug, Default)]
pub(super) struct BackendModel {
    entries: Vec<BackendEntry>,
}

impl BackendModel {
    pub(super) fn state(&self, registration: RegistrationId) -> ScriptedBackendState {
        self.entry_index(registration)
            .map_or(ScriptedBackendState::Absent, |index| {
                self.entries[index].state
            })
    }

    pub(super) fn begin_register(&mut self, registration: RegistrationId, descriptor: i32) {
        self.entries.push(BackendEntry {
            registration,
            descriptor,
            state: ScriptedBackendState::Absent,
        });
    }

    pub(super) fn complete_register(
        &mut self,
        registration: RegistrationId,
        interest: Interest,
        mode: Mode,
        outcome: MutationOutcome,
    ) {
        let state = match outcome {
            MutationOutcome::Success
            | MutationOutcome::Failure {
                commit: CommitStatus::Applied,
                ..
            } => ScriptedBackendState::Registered {
                interest,
                mode,
                arm: ArmState::Armed,
            },
            MutationOutcome::Failure {
                commit: CommitStatus::Unknown,
                ..
            } => ScriptedBackendState::Unknown,
            MutationOutcome::Failure {
                commit: CommitStatus::NotApplied,
                ..
            } => ScriptedBackendState::Absent,
        };
        self.set_state(registration, state);
    }

    pub(super) fn validate(
        &self,
        registration: RegistrationId,
        descriptor: i32,
    ) -> Result<(), ScriptError> {
        let index = self
            .entry_index(registration)
            .ok_or(ScriptError::UnknownRegistration { registration })?;
        if self.entries[index].descriptor == descriptor {
            Ok(())
        } else {
            Err(ScriptError::DescriptorChanged { registration })
        }
    }

    pub(super) fn complete_modify(
        &mut self,
        registration: RegistrationId,
        interest: Interest,
        mode: Mode,
        outcome: MutationOutcome,
    ) {
        match outcome {
            MutationOutcome::Success
            | MutationOutcome::Failure {
                commit: CommitStatus::Applied,
                ..
            } => self.set_state(
                registration,
                ScriptedBackendState::Registered {
                    interest,
                    mode,
                    arm: ArmState::Armed,
                },
            ),
            MutationOutcome::Failure {
                commit: CommitStatus::Unknown,
                ..
            } => self.set_state(registration, ScriptedBackendState::Unknown),
            MutationOutcome::Failure {
                commit: CommitStatus::NotApplied,
                ..
            } => {}
        }
    }

    pub(super) fn complete_delete(
        &mut self,
        registration: RegistrationId,
        outcome: MutationOutcome,
    ) {
        match outcome {
            MutationOutcome::Success
            | MutationOutcome::Failure {
                commit: CommitStatus::Applied,
                ..
            } => self.set_state(registration, ScriptedBackendState::Absent),
            MutationOutcome::Failure {
                commit: CommitStatus::Unknown,
                ..
            } => self.set_state(registration, ScriptedBackendState::Unknown),
            MutationOutcome::Failure {
                commit: CommitStatus::NotApplied,
                ..
            } => {}
        }
    }

    pub(super) fn establish_disarmed(
        &mut self,
        registration: RegistrationId,
        descriptor: i32,
    ) -> Result<(), ScriptError> {
        self.validate(registration, descriptor)?;
        let ScriptedBackendState::Registered {
            interest,
            mode: Mode::OneShot,
            ..
        } = self.state(registration)
        else {
            return Err(ScriptError::CannotDisarm { registration });
        };
        self.set_state(
            registration,
            ScriptedBackendState::Registered {
                interest,
                mode: Mode::OneShot,
                arm: ArmState::Disarmed,
            },
        );
        Ok(())
    }

    pub(super) fn mark_unknown(&mut self, registration: RegistrationId, descriptor: i32) {
        if let Some(index) = self.entry_index(registration) {
            self.entries[index].state = ScriptedBackendState::Unknown;
        } else {
            self.entries.push(BackendEntry {
                registration,
                descriptor,
                state: ScriptedBackendState::Unknown,
            });
        }
    }

    fn set_state(&mut self, registration: RegistrationId, state: ScriptedBackendState) {
        if let Some(index) = self.entry_index(registration) {
            self.entries[index].state = state;
        }
    }

    fn entry_index(&self, registration: RegistrationId) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.registration == registration)
    }
}
