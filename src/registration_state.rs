//! Registration-state inspection.

use crate::{ArmState, RegistrationState};

impl RegistrationState {
    /// Returns whether backend state is known to remain registered.
    pub const fn is_registered(self) -> bool {
        matches!(self, Self::Registered { .. })
    }

    /// Returns whether backend state cannot be proven.
    pub const fn is_uncertain(self) -> bool {
        matches!(self, Self::Uncertain)
    }

    /// Returns delivery eligibility when backend state is known.
    pub const fn arm(self) -> Option<ArmState> {
        match self {
            Self::Registered { arm } => Some(arm),
            Self::Uncertain => None,
        }
    }
}
