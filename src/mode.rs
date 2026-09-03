//! Readiness delivery modes.

/// Delivery behavior for one registered resource.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Mode {
    /// Continue reporting readiness while the resource remains ready.
    Level,
    /// Disarm after delivery until explicitly rearmed or modified.
    OneShot,
}

impl Mode {
    /// Returns whether delivery disarms the registration.
    pub const fn is_one_shot(self) -> bool {
        matches!(self, Self::OneShot)
    }
}

#[cfg(test)]
#[path = "mode_test.rs"]
mod tests;
