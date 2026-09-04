//! Semver-exempt wait instrumentation for qualification binaries.

use crate::Poll;

/// Native work retained from the most recent [`Poll::wait`](crate::Poll::wait).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WaitMetrics {
    native_observations: usize,
    one_shot_disarms: usize,
    disarm_elapsed_ns: u128,
}

impl WaitMetrics {
    /// Returns the number of raw events returned by the native wait.
    pub const fn native_observations(self) -> usize {
        self.native_observations
    }

    /// Returns the number of registrations in the one-shot disarm submission.
    pub const fn one_shot_disarms(self) -> usize {
        self.one_shot_disarms
    }

    /// Returns elapsed nanoseconds for the receipt-checked disarm submission.
    pub const fn disarm_elapsed_ns(self) -> u128 {
        self.disarm_elapsed_ns
    }
}

/// Returns native work retained from `poll`'s most recent wait.
pub const fn last_wait_metrics(poll: &Poll) -> WaitMetrics {
    let (native_observations, one_shot_disarms, disarm_elapsed_ns) = poll.test_wait_metrics;
    WaitMetrics {
        native_observations,
        one_shot_disarms,
        disarm_elapsed_ns,
    }
}
