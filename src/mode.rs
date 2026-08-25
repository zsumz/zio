//! Readiness delivery modes.

/// Delivery behavior for one registered resource.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Mode {
    /// Continue reporting readiness while the resource remains ready.
    Level,
    /// Disarm after delivery until an explicit modification rearms the resource.
    OneShot,
}
