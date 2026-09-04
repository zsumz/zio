//! Generator state and coverage transitions after each planned action.

use zio::{ArmState, Mode};

use crate::{
    model_sequence::{Action, Outcome},
    model_sequence_coverage::Coverage,
};

#[derive(Clone, Copy)]
pub(crate) enum GeneratedState {
    Vacant,
    Registered { mode: Mode, arm: ArmState },
    Uncertain,
}

pub(crate) fn observe(
    state: &mut GeneratedState,
    has_stale: &mut bool,
    coverage: &mut Coverage,
    action: Action,
) {
    match action {
        Action::Register { outcome, mode, .. } => {
            coverage.register[outcome.index()] = true;
            if *has_stale && outcome != Outcome::NotApplied {
                coverage.mark(Coverage::REUSE);
            }
            *state = match outcome {
                Outcome::Success | Outcome::Applied => GeneratedState::Registered {
                    mode,
                    arm: ArmState::Armed,
                },
                Outcome::NotApplied => GeneratedState::Vacant,
                Outcome::Unknown => GeneratedState::Uncertain,
            };
        }
        Action::RegisterInvalid { .. } => coverage.mark(Coverage::INVALID_REGISTER),
        Action::Disarm => {
            coverage.mark(Coverage::DISARM);
            if let GeneratedState::Registered { mode, .. } = *state {
                *state = GeneratedState::Registered {
                    mode,
                    arm: ArmState::Disarmed,
                };
            }
        }
        Action::SetKey { .. } => match *state {
            GeneratedState::Registered {
                arm: ArmState::Armed,
                ..
            } => coverage.mark(Coverage::SET_KEY_ARMED),
            GeneratedState::Registered {
                arm: ArmState::Disarmed,
                ..
            } => coverage.mark(Coverage::SET_KEY_DISARMED),
            GeneratedState::Uncertain => coverage.mark(Coverage::SET_KEY_UNCERTAIN),
            GeneratedState::Vacant => {}
        },
        Action::Modify { outcome, mode, .. } => {
            coverage.modify[outcome.index()] = true;
            observe_modify(state, coverage, outcome, mode);
        }
        Action::ModifyWithKey { outcome, mode, .. } => {
            coverage.modify_with_key[outcome.index()] = true;
            observe_modify(state, coverage, outcome, mode);
        }
        Action::ModifyInvalid { .. } => coverage.mark(Coverage::INVALID_MODIFY),
        Action::Delete { outcome } => {
            coverage.delete[outcome.index()] = true;
            *state = match outcome {
                Outcome::Success | Outcome::Applied => {
                    *has_stale = true;
                    GeneratedState::Vacant
                }
                Outcome::NotApplied => *state,
                Outcome::Unknown => GeneratedState::Uncertain,
            };
        }
        Action::ProbeStale => coverage.mark(Coverage::STALE),
        Action::ProbeWrongPoller => coverage.mark(Coverage::WRONG_POLLER),
    }
}

fn observe_modify(
    state: &mut GeneratedState,
    coverage: &mut Coverage,
    outcome: Outcome,
    mode: Mode,
) {
    if matches!(
        (*state, outcome),
        (
            GeneratedState::Registered {
                arm: ArmState::Disarmed,
                ..
            },
            Outcome::Success | Outcome::Applied
        )
    ) {
        coverage.mark(Coverage::REARM);
    }
    *state = match outcome {
        Outcome::Success | Outcome::Applied => GeneratedState::Registered {
            mode,
            arm: ArmState::Armed,
        },
        Outcome::NotApplied => *state,
        Outcome::Unknown => GeneratedState::Uncertain,
    };
}
