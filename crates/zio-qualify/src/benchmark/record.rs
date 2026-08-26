//! Per-round sample records and candidate rotation metadata.

use super::measure::Captured;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Sample {
    pub(crate) round: usize,
    pub(crate) order_position: usize,
    pub(crate) captured: Captured,
    pub(crate) retained_fd_delta: Option<i64>,
}
