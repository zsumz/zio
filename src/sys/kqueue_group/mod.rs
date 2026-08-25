//! Kqueue backend composition, policy, and reviewed syscall leaf.

#![cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]

mod backend;
#[cfg(test)]
mod backend_test;
mod kqueue;
mod kqueue_change;
mod kqueue_codec;
mod kqueue_disarm;
#[cfg(test)]
mod kqueue_disarm_matrix_test;
#[cfg(test)]
mod kqueue_disarm_test;
mod kqueue_policy;
#[cfg(test)]
mod kqueue_policy_test;
mod kqueue_timeout;

pub(crate) use backend::{Backend, RawBatch, Wake};
