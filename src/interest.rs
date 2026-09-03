//! Portable readiness interests.

use core::{
    fmt,
    ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Sub, SubAssign},
};

/// Backend-neutral readiness interests for one registration.
#[repr(transparent)]
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct Interest(u8);

impl Interest {
    /// No readiness interest.
    pub const EMPTY: Self = Self(0);
    /// Observe readable progress.
    pub const READABLE: Self = Self(1 << 0);
    /// Observe writable progress.
    pub const WRITABLE: Self = Self(1 << 1);
    /// Every supported readiness interest.
    pub const ALL: Self = Self::READABLE.union(Self::WRITABLE);

    /// Returns whether no readiness interest is present.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns whether every flag in `other` is present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns whether the sets share any interest.
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Returns the union of two interest sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns the interests present in both sets.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Returns the interests not present in `other`.
    #[must_use]
    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
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

impl BitOrAssign for Interest {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

impl BitAnd for Interest {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.intersection(rhs)
    }
}

impl BitAndAssign for Interest {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = self.intersection(rhs);
    }
}

impl Sub for Interest {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self.difference(rhs)
    }
}

impl SubAssign for Interest {
    fn sub_assign(&mut self, rhs: Self) {
        *self = self.difference(rhs);
    }
}

impl fmt::Debug for Interest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return formatter.write_str("EMPTY");
        }
        let mut separator = "";
        for (present, name) in [
            (self.is_readable(), "READABLE"),
            (self.is_writable(), "WRITABLE"),
        ] {
            if present {
                formatter.write_str(separator)?;
                formatter.write_str(name)?;
                separator = " | ";
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "interest_test.rs"]
mod tests;
