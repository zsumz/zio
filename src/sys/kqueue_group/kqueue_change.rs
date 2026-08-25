//! Fixed vocabulary for receipt-checked kqueue changes.

use std::{io, os::fd::RawFd};

/// Native kqueue filter selected for a change or observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Filter {
    Read,
    Write,
    User,
    Unknown,
}

/// Mutation requested for one `(ident, filter)` pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    AddEnabled,
    AddDisabled,
    AddUser,
    Delete,
    Disable,
    Trigger,
}

/// One platform-neutral input change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Change {
    ident: RawFd,
    filter: Filter,
    action: Action,
    token: u64,
}

impl Change {
    pub(super) const fn new(ident: RawFd, filter: Filter, action: Action, token: u64) -> Self {
        Self {
            ident,
            filter,
            action,
            token,
        }
    }

    pub(super) const fn ident(self) -> RawFd {
        self.ident
    }

    pub(super) const fn filter(self) -> Filter {
        self.filter
    }

    pub(super) const fn action(self) -> Action {
        self.action
    }

    pub(super) const fn token(self) -> u64 {
        self.token
    }
}

/// Fixed stack change list; one descriptor has at most two filters.
#[derive(Debug)]
pub(super) struct ChangeList {
    items: [Change; 2],
    len: usize,
}

impl ChangeList {
    pub(super) const fn new() -> Self {
        Self {
            items: [Change::new(-1, Filter::Unknown, Action::Delete, 0); 2],
            len: 0,
        }
    }

    pub(super) fn push(&mut self, change: Change) -> Option<()> {
        let destination = self.items.get_mut(self.len)?;
        *destination = change;
        self.len += 1;
        Some(())
    }

    pub(super) fn as_slice(&self) -> &[Change] {
        &self.items[..self.len]
    }
}

/// Receipt for one submitted change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Receipt {
    action: Action,
    error: Option<i32>,
}

impl Receipt {
    pub(super) const fn new(action: Action, error: Option<i32>) -> Self {
        Self { action, error }
    }

    pub(super) const fn action(self) -> Action {
        self.action
    }

    pub(super) const fn error(self) -> Option<i32> {
        self.error
    }
}

/// Fixed receipt set matching one change list.
#[derive(Debug)]
pub(super) struct Receipts {
    items: [Receipt; 2],
    len: usize,
}

impl Receipts {
    pub(super) const fn new(len: usize) -> Self {
        Self {
            items: [Receipt::new(Action::Delete, None); 2],
            len,
        }
    }

    pub(super) fn set(&mut self, index: usize, receipt: Receipt) -> io::Result<()> {
        let destination = self
            .items
            .get_mut(index)
            .ok_or_else(|| io::Error::other("kqueue receipt index overflowed"))?;
        *destination = receipt;
        Ok(())
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = Receipt> + '_ {
        self.items[..self.len].iter().copied()
    }
}

/// Copied kevent observation detached from kernel-written storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RawKevent {
    ident: RawFd,
    filter: Filter,
    token: u64,
    eof: bool,
    native_error: bool,
    fflags: u32,
}

impl RawKevent {
    pub(super) const fn new(
        ident: RawFd,
        filter: Filter,
        token: u64,
        eof: bool,
        native_error: bool,
        fflags: u32,
    ) -> Self {
        Self {
            ident,
            filter,
            token,
            eof,
            native_error,
            fflags,
        }
    }

    pub(super) const fn ident(self) -> RawFd {
        self.ident
    }

    pub(super) const fn filter(self) -> Filter {
        self.filter
    }

    pub(super) const fn token(self) -> u64 {
        self.token
    }

    pub(super) const fn eof(self) -> bool {
        self.eof
    }

    pub(super) const fn native_error(self) -> bool {
        self.native_error
    }

    pub(super) const fn fflags(self) -> u32 {
        self.fflags
    }
}
