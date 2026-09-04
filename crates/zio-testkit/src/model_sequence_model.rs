//! Tiny independent reference model for registration lifetime and mutations.

use zio::test_support::{MutationCall, ScriptedBackendState};
use zio::{ArmState, Interest, Key, Mode, Registration, RegistrationId, RegistrationState};

use crate::model_sequence::{ACTION_LIMIT, Outcome};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExpectedState {
    Registered { arm: ArmState },
    Uncertain,
}

impl ExpectedState {
    const ARMED: Self = Self::Registered {
        arm: ArmState::Armed,
    };

    pub(crate) const fn portable(self) -> RegistrationState {
        match self {
            Self::Registered { arm } => RegistrationState::Registered { arm },
            Self::Uncertain => RegistrationState::Uncertain,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Entry {
    pub(crate) registration: Registration,
    pub(crate) key: Key,
    pub(crate) interest: Interest,
    pub(crate) mode: Mode,
    pub(crate) state: ExpectedState,
}

impl Entry {
    pub(crate) const fn backend(self) -> ScriptedBackendState {
        match self.state {
            ExpectedState::Registered { arm } => ScriptedBackendState::Registered {
                interest: self.interest,
                mode: self.mode,
                arm,
            },
            ExpectedState::Uncertain => ScriptedBackendState::Unknown,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ReferenceModel {
    active: Option<Entry>,
    retired: Vec<Registration>,
    issued: Vec<RegistrationId>,
    calls: Vec<MutationCall>,
}

impl ReferenceModel {
    pub(crate) fn new() -> Result<Self, ()> {
        let mut retired = Vec::new();
        retired.try_reserve_exact(ACTION_LIMIT).map_err(|_| ())?;
        let mut issued = Vec::new();
        issued.try_reserve_exact(ACTION_LIMIT).map_err(|_| ())?;
        let mut calls = Vec::new();
        calls.try_reserve_exact(ACTION_LIMIT).map_err(|_| ())?;
        Ok(Self {
            active: None,
            retired,
            issued,
            calls,
        })
    }

    pub(crate) const fn active(&self) -> Option<Entry> {
        self.active
    }

    pub(crate) fn stale(&self) -> Option<Registration> {
        self.retired.last().copied()
    }

    pub(crate) fn issued(&self) -> &[RegistrationId] {
        &self.issued
    }

    pub(crate) fn retired(&self) -> &[Registration] {
        &self.retired
    }

    pub(crate) fn calls(&self) -> &[MutationCall] {
        &self.calls
    }

    pub(crate) fn record_register(
        &mut self,
        registration: RegistrationId,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), &'static str> {
        if self.active.is_some() {
            return Err("register requires a vacant reference slot");
        }
        if self.issued.contains(&registration) {
            return Err("register repeated an issued generation");
        }
        self.issued.push(registration);
        self.calls.push(MutationCall::Register {
            registration,
            key,
            interest,
            mode,
        });
        Ok(())
    }

    pub(crate) fn complete_register(
        &mut self,
        outcome: Outcome,
        registration: Option<Registration>,
        key: Key,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), &'static str> {
        let state = match outcome {
            Outcome::NotApplied => {
                if registration.is_some() {
                    return Err("not-applied register retained a handle");
                }
                self.active = None;
                return Ok(());
            }
            Outcome::Success | Outcome::Applied => ExpectedState::ARMED,
            Outcome::Unknown => ExpectedState::Uncertain,
        };
        self.active = Some(Entry {
            registration: registration.ok_or("retaining register omitted its handle")?,
            key,
            interest,
            mode,
            state,
        });
        Ok(())
    }

    pub(crate) fn set_key(&mut self, key: Key) -> Result<(), &'static str> {
        let entry = self.active.as_mut().ok_or("set_key requires a handle")?;
        entry.key = key;
        Ok(())
    }

    pub(crate) fn disarm(&mut self) -> Result<RegistrationId, &'static str> {
        let entry = self.active.as_mut().ok_or("disarm requires a handle")?;
        if entry.mode != Mode::OneShot || entry.state != ExpectedState::ARMED {
            return Err("disarm requires an armed one-shot registration");
        }
        let registration = entry.registration.id();
        self.calls
            .push(MutationCall::EstablishDisarmed { registration });
        entry.state = ExpectedState::Registered {
            arm: ArmState::Disarmed,
        };
        Ok(registration)
    }

    pub(crate) fn record_modify(
        &mut self,
        desired_interest: Interest,
        desired_mode: Mode,
    ) -> Result<Entry, &'static str> {
        let entry = self.active.ok_or("modify requires a handle")?;
        let ExpectedState::Registered { arm } = entry.state else {
            return Err("modify requires a proven registered state");
        };
        self.calls.push(MutationCall::Modify {
            registration: entry.registration.id(),
            previous_interest: entry.interest,
            previous_mode: entry.mode,
            previous_arm: arm,
            desired_interest,
            desired_mode,
        });
        Ok(entry)
    }

    pub(crate) fn complete_modify(
        &mut self,
        outcome: Outcome,
        key: Option<Key>,
        interest: Interest,
        mode: Mode,
    ) -> Result<(), &'static str> {
        let entry = self.active.as_mut().ok_or("modify lost active handle")?;
        match outcome {
            Outcome::Success | Outcome::Applied => {
                if let Some(key) = key {
                    entry.key = key;
                }
                entry.interest = interest;
                entry.mode = mode;
                entry.state = ExpectedState::ARMED;
            }
            Outcome::NotApplied => {}
            Outcome::Unknown => entry.state = ExpectedState::Uncertain,
        }
        Ok(())
    }

    pub(crate) fn record_delete(&mut self) -> Result<Entry, &'static str> {
        let entry = self.active.ok_or("delete requires a handle")?;
        self.calls.push(MutationCall::Delete {
            registration: entry.registration.id(),
            interest: entry.interest,
            state: entry.state.portable(),
        });
        Ok(entry)
    }

    pub(crate) fn complete_delete(&mut self, outcome: Outcome) -> Result<(), &'static str> {
        match outcome {
            Outcome::Success | Outcome::Applied => {
                let entry = self.active.take().ok_or("delete lost its active handle")?;
                self.retired.push(entry.registration);
            }
            Outcome::NotApplied => {}
            Outcome::Unknown => {
                let entry = self.active.as_mut().ok_or("delete lost active handle")?;
                entry.state = ExpectedState::Uncertain;
            }
        }
        Ok(())
    }

    pub(crate) fn expected_backend(&self, id: RegistrationId) -> ScriptedBackendState {
        match self.active {
            Some(entry) if entry.registration.id() == id => entry.backend(),
            _ => ScriptedBackendState::Absent,
        }
    }
}
