//! Exact kqueue storage sizing and failure attribution.

use crate::error::{CapacityKind, CapacityReason, Error};

#[allow(dead_code, reason = "projection-stable kqueue capacity plan")]
#[derive(Clone, Copy, Debug)]
pub(crate) struct KqueueCapacity {
    events: usize,
    registrations: usize,
    native_events: usize,
    native_changes: usize,
    recoveries: usize,
    arena_uses_events: bool,
    recovery_uses_events: bool,
}

#[allow(dead_code, reason = "projection-stable kqueue capacity plan")]
impl KqueueCapacity {
    pub(crate) fn new(events: usize, registrations: usize) -> Result<Self, Error> {
        let recovery_uses_events = events < registrations;
        let limit_error = |uses_events| {
            capacity_error(
                uses_events,
                events,
                registrations,
                CapacityReason::BackendLimit,
            )
        };
        let native_events = registrations
            .checked_mul(2)
            .and_then(|capacity| capacity.checked_add(1))
            .ok_or_else(|| limit_error(false))?;
        let recoveries = events.min(registrations);
        let native_changes = recoveries
            .checked_mul(2)
            .ok_or_else(|| limit_error(recovery_uses_events))?;
        let recovery_storage = native_changes
            .checked_mul(2)
            .ok_or_else(|| limit_error(recovery_uses_events))?;
        #[cfg(not(target_os = "netbsd"))]
        {
            i32::try_from(native_events).map_err(|_| limit_error(false))?;
            i32::try_from(native_changes).map_err(|_| limit_error(recovery_uses_events))?;
        }
        Ok(Self {
            events,
            registrations,
            native_events,
            native_changes,
            recoveries,
            arena_uses_events: recovery_uses_events && recovery_storage > native_events,
            recovery_uses_events,
        })
    }

    pub(crate) const fn native_events(self) -> usize {
        self.native_events
    }

    pub(crate) const fn native_changes(self) -> usize {
        self.native_changes
    }

    pub(crate) const fn recoveries(self) -> usize {
        self.recoveries
    }

    pub(crate) const fn arena_error(self) -> Error {
        capacity_error(
            self.arena_uses_events,
            self.events,
            self.registrations,
            CapacityReason::StorageUnavailable,
        )
    }

    pub(crate) const fn recovery_error(self) -> Error {
        capacity_error(
            self.recovery_uses_events,
            self.events,
            self.registrations,
            CapacityReason::StorageUnavailable,
        )
    }
}

const fn capacity_error(
    uses_events: bool,
    events: usize,
    registrations: usize,
    reason: CapacityReason,
) -> Error {
    let (kind, limit) = if uses_events {
        (CapacityKind::Event, events)
    } else {
        (CapacityKind::Registration, registrations)
    };
    Error::Capacity {
        kind,
        limit,
        reason,
    }
}
