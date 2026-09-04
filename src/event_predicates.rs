//! Direct event classification and readiness queries.

use super::Event;

impl Event {
    /// Returns whether this is a resource observation.
    pub const fn is_resource(self) -> bool {
        matches!(self, Self::Resource { .. })
    }

    /// Returns whether this is an explicit wake observation.
    pub const fn is_wake(self) -> bool {
        matches!(self, Self::Wake { .. })
    }

    /// Returns whether readable readiness is present.
    pub const fn is_readable(self) -> bool {
        matches!(self.readiness(), Some(readiness) if readiness.is_readable())
    }

    /// Returns whether writable readiness is present.
    pub const fn is_writable(self) -> bool {
        matches!(self.readiness(), Some(readiness) if readiness.is_writable())
    }

    /// Returns whether readable closure is present.
    pub const fn is_read_closed(self) -> bool {
        matches!(self.readiness(), Some(readiness) if readiness.is_read_closed())
    }

    /// Returns whether writable closure is present.
    pub const fn is_write_closed(self) -> bool {
        matches!(self.readiness(), Some(readiness) if readiness.is_write_closed())
    }

    /// Returns whether an error hint is present.
    pub const fn is_error(self) -> bool {
        matches!(self.readiness(), Some(readiness) if readiness.is_error())
    }
}
