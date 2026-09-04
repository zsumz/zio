//! Readiness delivery modes.

/// Delivery behavior for one registered resource.
///
/// Future zio 1.x releases may add delivery modes. Match with a fallback arm.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Mode {
    /// Continue reporting readiness while the resource remains ready.
    Level,
    /// Disarm after delivery until explicitly rearmed or modified.
    OneShot,
}

impl Mode {
    #[cfg_attr(
        not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )),
        allow(dead_code, reason = "only native backends classify delivery modes")
    )]
    pub(crate) const fn is_one_shot(self) -> bool {
        matches!(self, Self::OneShot)
    }
}

#[cfg(test)]
#[path = "mode_test.rs"]
mod tests;
