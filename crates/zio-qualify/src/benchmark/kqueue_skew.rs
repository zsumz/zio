//! Dedicated kqueue scaling receipts for skewed configured capacities.

#[path = "kqueue_skew/config.rs"]
mod config;
#[cfg(test)]
#[path = "kqueue_skew/config_test.rs"]
mod config_test;
#[path = "kqueue_skew/fixture.rs"]
#[allow(
    dead_code,
    reason = "non-kqueue builds retain the runner for cross-target compilation"
)]
mod fixture;
#[path = "kqueue_skew/model.rs"]
mod model;
#[path = "kqueue_skew/receipt.rs"]
mod receipt;
#[cfg(test)]
#[path = "kqueue_skew/receipt_test.rs"]
mod receipt_test;
#[path = "kqueue_skew/resource.rs"]
mod resource;
#[path = "kqueue_skew/runner.rs"]
mod runner;

pub use runner::run;
