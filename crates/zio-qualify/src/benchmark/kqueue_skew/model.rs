//! Receipt-facing kqueue skew measurement state.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RetainedMemory {
    pub(super) allocation_count: i64,
    pub(super) bytes: i64,
    pub(super) peak_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Measurement {
    pub(super) elapsed_ns: u128,
    pub(super) waits: u64,
    pub(super) native_observations: u64,
    pub(super) logical_events: u64,
    pub(super) unique_registrations: u64,
    pub(super) disarm_submissions: u64,
    pub(super) disarmed_registrations: u64,
    pub(super) disarm_elapsed_ns: u128,
    pub(super) retained_memory: RetainedMemory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Resources {
    pub(super) open_fds: Option<u64>,
    pub(super) soft_fd_limit: Option<u64>,
    pub(super) fd_limit_source: Option<&'static str>,
    pub(super) required_additional_fds: u64,
}

#[allow(
    dead_code,
    reason = "non-kqueue hosts construct only the structured unsupported variant"
)]
#[allow(
    clippy::large_enum_variant,
    reason = "one bounded benchmark-row outcome is serialized at a time; \
              boxing the measurements would add unnecessary indirection"
)]
pub(super) enum Outcome {
    Passed {
        level: Measurement,
        one_shot: Measurement,
    },
    Unsupported {
        code: &'static str,
        reason: String,
    },
    Failed(String),
}
