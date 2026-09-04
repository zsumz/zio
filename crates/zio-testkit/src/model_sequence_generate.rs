//! Dependency-free deterministic generation with explicit preconditions.

use zio::{ArmState, Interest, Key, Mode};

use crate::{
    model_sequence::{ACTION_LIMIT, Action, Outcome, SEED_GAMMA, scramble},
    model_sequence_coverage::Coverage,
};

#[derive(Clone, Debug)]
pub(crate) struct GeneratedProgram {
    pub(crate) actions: Vec<Action>,
    pub(crate) coverage: Coverage,
}

#[derive(Clone, Copy)]
enum GeneratedState {
    Vacant,
    Registered { mode: Mode, arm: ArmState },
    Uncertain,
}

pub(crate) fn generate(seed: u64) -> Result<GeneratedProgram, ()> {
    let mut generator = Generator::new(seed);
    let mut actions = Vec::new();
    actions.try_reserve_exact(ACTION_LIMIT).map_err(|_| ())?;
    for _ in 0..ACTION_LIMIT {
        actions.push(generator.next_action());
    }
    Ok(GeneratedProgram {
        actions,
        coverage: generator.coverage,
    })
}

struct Generator {
    random: SplitMix64,
    state: GeneratedState,
    has_stale: bool,
    coverage: Coverage,
}

impl Generator {
    const fn new(seed: u64) -> Self {
        Self {
            random: SplitMix64(seed),
            state: GeneratedState::Vacant,
            has_stale: false,
            coverage: Coverage {
                register: [false; 4],
                modify: [false; 4],
                delete: [false; 4],
                special: 0,
            },
        }
    }

    fn next_action(&mut self) -> Action {
        let state = self.state;
        let action = match state {
            GeneratedState::Vacant if self.choose(6) == 0 => Action::RegisterInvalid {
                key: Key::new(self.random.next()),
                mode: self.mode(),
            },
            GeneratedState::Vacant => self.register(),
            GeneratedState::Registered { mode, arm }
                if mode == Mode::OneShot && arm == ArmState::Armed && self.choose(8) == 0 =>
            {
                Action::Disarm
            }
            GeneratedState::Registered {
                arm: ArmState::Disarmed,
                ..
            } if self.choose(4) == 0 => self.set_key(),
            GeneratedState::Registered { .. } => match self.choose(10) {
                0..=2 => self.modify(),
                3 | 4 => self.delete(),
                5 => Action::ProbeWrongPoller,
                6 => Action::ModifyInvalid { mode: self.mode() },
                7 => self.set_key(),
                _ if self.has_stale => Action::ProbeStale,
                _ => self.delete(),
            },
            GeneratedState::Uncertain => match self.choose(8) {
                0..=4 => self.delete(),
                5 => Action::ProbeWrongPoller,
                6 => self.set_key(),
                _ if self.has_stale => Action::ProbeStale,
                _ => self.delete(),
            },
        };
        self.observe(action);
        action
    }

    fn register(&mut self) -> Action {
        Action::Register {
            outcome: self.outcome(),
            key: Key::new(self.random.next()),
            interest: self.interest(),
            mode: self.mode(),
        }
    }

    fn modify(&mut self) -> Action {
        Action::Modify {
            outcome: self.outcome(),
            interest: self.interest(),
            mode: self.mode(),
        }
    }

    fn set_key(&mut self) -> Action {
        Action::SetKey {
            key: Key::new(self.random.next()),
        }
    }

    fn delete(&mut self) -> Action {
        Action::Delete {
            outcome: self.outcome(),
        }
    }

    fn observe(&mut self, action: Action) {
        match action {
            Action::Register { outcome, mode, .. } => {
                self.coverage.register[outcome.index()] = true;
                if self.has_stale && outcome != Outcome::NotApplied {
                    self.coverage.mark(Coverage::REUSE);
                }
                self.state = match outcome {
                    Outcome::Success | Outcome::Applied => GeneratedState::Registered {
                        mode,
                        arm: ArmState::Armed,
                    },
                    Outcome::NotApplied => GeneratedState::Vacant,
                    Outcome::Unknown => GeneratedState::Uncertain,
                };
            }
            Action::RegisterInvalid { .. } => self.coverage.mark(Coverage::INVALID_REGISTER),
            Action::Disarm => {
                self.coverage.mark(Coverage::DISARM);
                if let GeneratedState::Registered { mode, .. } = self.state {
                    self.state = GeneratedState::Registered {
                        mode,
                        arm: ArmState::Disarmed,
                    };
                }
            }
            Action::SetKey { .. } => match self.state {
                GeneratedState::Registered {
                    arm: ArmState::Armed,
                    ..
                } => self.coverage.mark(Coverage::SET_KEY_ARMED),
                GeneratedState::Registered {
                    arm: ArmState::Disarmed,
                    ..
                } => self.coverage.mark(Coverage::SET_KEY_DISARMED),
                GeneratedState::Uncertain => self.coverage.mark(Coverage::SET_KEY_UNCERTAIN),
                GeneratedState::Vacant => {}
            },
            Action::Modify { outcome, mode, .. } => {
                self.coverage.modify[outcome.index()] = true;
                if matches!(
                    (self.state, outcome),
                    (
                        GeneratedState::Registered {
                            arm: ArmState::Disarmed,
                            ..
                        },
                        Outcome::Success | Outcome::Applied
                    )
                ) {
                    self.coverage.mark(Coverage::REARM);
                }
                self.state = match outcome {
                    Outcome::Success | Outcome::Applied => GeneratedState::Registered {
                        mode,
                        arm: ArmState::Armed,
                    },
                    Outcome::NotApplied => self.state,
                    Outcome::Unknown => GeneratedState::Uncertain,
                };
            }
            Action::ModifyInvalid { .. } => self.coverage.mark(Coverage::INVALID_MODIFY),
            Action::Delete { outcome } => {
                self.coverage.delete[outcome.index()] = true;
                self.state = match outcome {
                    Outcome::Success | Outcome::Applied => {
                        self.has_stale = true;
                        GeneratedState::Vacant
                    }
                    Outcome::NotApplied => self.state,
                    Outcome::Unknown => GeneratedState::Uncertain,
                };
            }
            Action::ProbeStale => self.coverage.mark(Coverage::STALE),
            Action::ProbeWrongPoller => self.coverage.mark(Coverage::WRONG_POLLER),
        }
    }

    fn outcome(&mut self) -> Outcome {
        match self.choose(4) {
            0 => Outcome::Success,
            1 => Outcome::NotApplied,
            2 => Outcome::Applied,
            _ => Outcome::Unknown,
        }
    }

    fn interest(&mut self) -> Interest {
        match self.choose(3) {
            0 => Interest::READABLE,
            1 => Interest::WRITABLE,
            _ => Interest::READABLE.union(Interest::WRITABLE),
        }
    }

    fn mode(&mut self) -> Mode {
        if self.choose(2) == 0 {
            Mode::Level
        } else {
            Mode::OneShot
        }
    }

    fn choose(&mut self, options: u64) -> u64 {
        self.random.next() % options
    }
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(SEED_GAMMA);
        scramble(self.0)
    }
}
