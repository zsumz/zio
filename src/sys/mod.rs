//! Target-selected readiness, wake, and selector mechanisms.

mod event;
mod failure;
mod kqueue_group;
mod linux_group;
mod platform;
mod raw_batch;
#[cfg(test)]
mod raw_batch_test;
mod unsupported;
mod wake;

pub(crate) use failure::MutationFailure;
pub(crate) use platform::Backend;
pub(crate) use raw_batch::RawBatch;
pub(crate) use wake::Wake;
