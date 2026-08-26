//! Vendor-neutral observations and contract policy.

use core::{fmt, ops::BitOr};

/// Vendor-neutral readiness hints.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Observation(u8);

impl Observation {
    /// No hints.
    pub const EMPTY: Self = Self(0);
    /// A read may make progress.
    pub const READABLE: Self = Self(1 << 0);
    /// A write may make progress.
    pub const WRITABLE: Self = Self(1 << 1);
    /// The readable direction reported a terminal condition.
    pub const READ_CLOSED: Self = Self(1 << 2);
    /// The writable direction reported a terminal condition.
    pub const WRITE_CLOSED: Self = Self(1 << 3);
    /// A resource error was reported.
    pub const ERROR: Self = Self(1 << 4);
    /// A direction-neutral interruption or hangup was reported.
    pub const INTERRUPT: Self = Self(1 << 5);

    const FLAGS: [(Self, &'static str); 6] = [
        (Self::READABLE, "readable"),
        (Self::WRITABLE, "writable"),
        (Self::READ_CLOSED, "read_closed"),
        (Self::WRITE_CLOSED, "write_closed"),
        (Self::ERROR, "error"),
        (Self::INTERRUPT, "interrupt"),
    ];

    /// Returns whether no hint is present.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns whether every flag in `other` is present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns whether any flag in `other` is present.
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Returns the union of two hint sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BitOr for Observation {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl fmt::Display for Observation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return formatter.write_str("empty");
        }
        let mut separator = "";
        for (flag, name) in Self::FLAGS {
            if self.contains(flag) {
                formatter.write_str(separator)?;
                formatter.write_str(name)?;
                separator = "|";
            }
        }
        Ok(())
    }
}

/// A contract's required minimum and complete allowance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedObservation {
    required: Observation,
    required_any: Observation,
    allowed: Observation,
}

impl ExpectedObservation {
    /// Creates a declared observation contract.
    pub const fn new(
        required: Observation,
        required_any: Observation,
        allowed: Observation,
    ) -> Self {
        Self {
            required,
            required_any,
            allowed,
        }
    }

    /// Returns the flags required together.
    pub const fn required(self) -> Observation {
        self.required
    }

    /// Returns the flags from which at least one is required.
    pub const fn required_any(self) -> Observation {
        self.required_any
    }

    /// Returns the complete documented allowance.
    pub const fn allowed(self) -> Observation {
        self.allowed
    }

    /// Validates one observation without consulting another candidate.
    pub const fn validate(self, actual: Observation) -> Result<(), ContractViolation> {
        if !actual.contains(self.required) {
            return Err(ContractViolation::MissingRequired {
                required: self.required,
                actual,
            });
        }
        if !self.required_any.is_empty() && !actual.intersects(self.required_any) {
            return Err(ContractViolation::MissingOneOf {
                required_any: self.required_any,
                actual,
            });
        }
        if !self.allowed.contains(actual) {
            return Err(ContractViolation::Undocumented {
                allowed: self.allowed,
                actual,
            });
        }
        Ok(())
    }
}

/// The precise way an observation failed its contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractViolation {
    /// A required flag was absent.
    MissingRequired {
        /// Required flags.
        required: Observation,
        /// Actual hints.
        actual: Observation,
    },
    /// None of the required alternatives was present.
    MissingOneOf {
        /// Alternative flags.
        required_any: Observation,
        /// Actual hints.
        actual: Observation,
    },
    /// An undeclared flag was present.
    Undocumented {
        /// Complete allowance.
        allowed: Observation,
        /// Actual hints.
        actual: Observation,
    },
}
