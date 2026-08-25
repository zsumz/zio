//! Stable names and branch vocabulary for mutation scenarios.

use zio::{CommitStatus, Operation};

/// One of the three registration mutations covered by the suite.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MutationOperation {
    /// Acquire and install a registration.
    Register,
    /// Replace registration configuration and rearm it.
    Modify,
    /// Remove and retire a registration.
    Delete,
}

impl MutationOperation {
    /// Returns the corresponding public zio operation.
    pub const fn operation(self) -> Operation {
        match self {
            Self::Register => Operation::Register,
            Self::Modify => Operation::Modify,
            Self::Delete => Operation::Delete,
        }
    }
}

/// Observable branch selected for a mutation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Branch {
    /// The backend mutation succeeds.
    Success,
    /// The mutation fails and is proven not to have applied.
    NotApplied,
    /// The mutation applies before a later failure is reported.
    Applied,
    /// The resulting backend state cannot be proven.
    Unknown,
}

impl Branch {
    /// Returns the failed branch's commit status.
    pub const fn commit(self) -> Option<CommitStatus> {
        match self {
            Self::Success => None,
            Self::NotApplied => Some(CommitStatus::NotApplied),
            Self::Applied => Some(CommitStatus::Applied),
            Self::Unknown => Some(CommitStatus::Unknown),
        }
    }
}

/// One stable mutation conformance scenario.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MutationScenario {
    operation: MutationOperation,
    branch: Branch,
}

impl MutationScenario {
    /// Every V1 mutation scenario in stable execution order.
    pub const ALL: [Self; 12] = [
        REGISTER_SUCCESS,
        REGISTER_NOT_APPLIED,
        REGISTER_APPLIED,
        REGISTER_UNKNOWN,
        MODIFY_SUCCESS,
        MODIFY_NOT_APPLIED,
        MODIFY_APPLIED,
        MODIFY_UNKNOWN,
        DELETE_SUCCESS,
        DELETE_NOT_APPLIED,
        DELETE_APPLIED,
        DELETE_UNKNOWN,
    ];

    /// Creates a scenario for an operation and branch.
    pub const fn new(operation: MutationOperation, branch: Branch) -> Self {
        Self { operation, branch }
    }

    /// Creates a register scenario.
    pub const fn register(branch: Branch) -> Self {
        Self::new(MutationOperation::Register, branch)
    }

    /// Creates a modify scenario.
    pub const fn modify(branch: Branch) -> Self {
        Self::new(MutationOperation::Modify, branch)
    }

    /// Creates a delete scenario.
    pub const fn delete(branch: Branch) -> Self {
        Self::new(MutationOperation::Delete, branch)
    }

    /// Returns the covered mutation.
    pub const fn operation(self) -> MutationOperation {
        self.operation
    }

    /// Returns the selected branch.
    pub const fn branch(self) -> Branch {
        self.branch
    }

    /// Returns the stable scenario name.
    pub const fn name(self) -> &'static str {
        match (self.operation, self.branch) {
            (MutationOperation::Register, Branch::Success) => "register.success",
            (MutationOperation::Register, Branch::NotApplied) => "register.not_applied",
            (MutationOperation::Register, Branch::Applied) => "register.applied",
            (MutationOperation::Register, Branch::Unknown) => "register.unknown",
            (MutationOperation::Modify, Branch::Success) => "modify.success",
            (MutationOperation::Modify, Branch::NotApplied) => "modify.not_applied",
            (MutationOperation::Modify, Branch::Applied) => "modify.applied",
            (MutationOperation::Modify, Branch::Unknown) => "modify.unknown",
            (MutationOperation::Delete, Branch::Success) => "delete.success",
            (MutationOperation::Delete, Branch::NotApplied) => "delete.not_applied",
            (MutationOperation::Delete, Branch::Applied) => "delete.applied",
            (MutationOperation::Delete, Branch::Unknown) => "delete.unknown",
        }
    }
}

/// Successful registration.
pub const REGISTER_SUCCESS: MutationScenario = MutationScenario::register(Branch::Success);
/// Registration proven not applied.
pub const REGISTER_NOT_APPLIED: MutationScenario = MutationScenario::register(Branch::NotApplied);
/// Registration applied before failure.
pub const REGISTER_APPLIED: MutationScenario = MutationScenario::register(Branch::Applied);
/// Registration with unknown backend state.
pub const REGISTER_UNKNOWN: MutationScenario = MutationScenario::register(Branch::Unknown);
/// Successful modification.
pub const MODIFY_SUCCESS: MutationScenario = MutationScenario::modify(Branch::Success);
/// Modification proven not applied.
pub const MODIFY_NOT_APPLIED: MutationScenario = MutationScenario::modify(Branch::NotApplied);
/// Modification applied before failure.
pub const MODIFY_APPLIED: MutationScenario = MutationScenario::modify(Branch::Applied);
/// Modification with unknown backend state.
pub const MODIFY_UNKNOWN: MutationScenario = MutationScenario::modify(Branch::Unknown);
/// Successful deletion.
pub const DELETE_SUCCESS: MutationScenario = MutationScenario::delete(Branch::Success);
/// Deletion proven not applied.
pub const DELETE_NOT_APPLIED: MutationScenario = MutationScenario::delete(Branch::NotApplied);
/// Deletion applied before failure.
pub const DELETE_APPLIED: MutationScenario = MutationScenario::delete(Branch::Applied);
/// Deletion with unknown backend state.
pub const DELETE_UNKNOWN: MutationScenario = MutationScenario::delete(Branch::Unknown);
