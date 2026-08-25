//! Linux backend composition and reviewed syscall leaves.

#![cfg(target_os = "linux")]

mod backend;
mod epoll;
mod eventfd;

pub(crate) use backend::{Backend, RawBatch, Wake};
