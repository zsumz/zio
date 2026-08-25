//! Portable readiness interests.

use core::ops::BitOr;

/// Backend-neutral readiness interests for one registration.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Interest(u8);

impl Interest {
    /// No readiness interest.
    pub const EMPTY: Self = Self(0);
    /// Observe readable progress.
    pub const READABLE: Self = Self(1 << 0);
    /// Observe writable progress.
    pub const WRITABLE: Self = Self(1 << 1);

    /// Returns whether no readiness interest is present.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns whether every flag in `other` is present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns the union of two interest sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns whether readable interest is present.
    pub const fn is_readable(self) -> bool {
        self.contains(Self::READABLE)
    }

    /// Returns whether writable interest is present.
    pub const fn is_writable(self) -> bool {
        self.contains(Self::WRITABLE)
    }
}

impl BitOr for Interest {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}
