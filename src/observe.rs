//! Bounded native waiting and portable event translation.

#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
use std::os::fd::AsRawFd;
use std::time::Instant;

use crate::{Error, Events, Operation, Poll, Wait, WaitReport};

impl Poll {
    /// Replaces `events` with one bounded readiness observation.
    ///
    /// `events` must match or exceed [`Self::event_capacity`]. It is cleared on
    /// entry and left empty on error. A successful report may carry one-shot
    /// recovery trouble; process every event before reconciling it.
    pub fn wait(&mut self, events: &mut Events, wait: Wait) -> Result<WaitReport, Error> {
        events.clear();
        self.pending.clear();
        let result = self.observe(events, wait);
        self.pending.clear();
        if result.is_err() {
            events.clear();
        }
        result
    }

    /// Replaces `events` with one observation bounded by `deadline`.
    ///
    /// A reached deadline is nonblocking. Backend interruption remains an
    /// [`Error::Io`] failure.
    pub fn wait_until(
        &mut self,
        events: &mut Events,
        deadline: Instant,
    ) -> Result<WaitReport, Error> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        self.wait(events, Wait::For(remaining))
    }

    fn observe(&mut self, events: &mut Events, wait: Wait) -> Result<WaitReport, Error> {
        if events.capacity() < self.event_capacity.get() {
            return Err(Error::EventsTooSmall {
                required: self.event_capacity.get(),
                actual: events.capacity(),
            });
        }
        let observed = self
            .backend
            .wait(&mut self.raw_events, events, wait)
            .map_err(|source| Error::Io {
                operation: Operation::Wait,
                source,
            })?;
        #[cfg(target_os = "linux")]
        {
            self.translate_linux(observed, events)?;
            Ok(WaitReport::new(None))
        }
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        {
            self.translate_kqueue(observed, events)
        }
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        {
            let _ = observed;
            Ok(WaitReport::new(None))
        }
    }

    #[cfg(target_os = "linux")]
    #[inline]
    fn translate_linux(&mut self, observed: usize, events: &mut Events) -> Result<(), Error> {
        let owner = self.owner.current();
        let registrations = &mut self.registrations;
        self.raw_events
            .translate_linux(events, observed, self.wake_key, |token| {
                let Some(resource) = registrations.resolve(token) else {
                    return Ok(None);
                };
                let registration =
                    crate::Registration::from_verified(owner.ok_or(Error::Invariant)?, resource.id);
                let _ = resource.descriptor;
                if resource.mode.is_one_shot() {
                    registrations.apply_disarm(resource.id, crate::CommitStatus::Applied)?;
                }
                Ok(Some((registration, resource.key)))
            })
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    fn translate_kqueue(
        &mut self,
        observed: usize,
        events: &mut Events,
    ) -> Result<WaitReport, Error> {
        let owner = self.owner.current();
        let mut woke = false;
        for index in 0..observed {
            let raw = self
                .raw_events
                .event(index, observed)
                .ok_or(Error::Invariant)?;
            if raw.is_control() {
                woke = true;
                continue;
            }
            let Some(resource) = self.registrations.resolve(raw.token()) else {
                continue;
            };
            if raw.descriptor() != resource.descriptor {
                continue;
            }
            self.pending
                .add(resource.id, resource.key, raw.readiness())?;
        }
        let resource_limit = self.event_capacity.get() - usize::from(woke);
        let delivered = self.pending.as_slice().len().min(resource_limit);
        self.prepare_disarms(delivered)?;
        let recovery = self.backend.submit_disarms(&mut self.raw_events).err();
        crate::observe_recovery::finish(
            owner,
            &mut self.registrations,
            events,
            self.pending.as_slice(),
            delivered,
            woke,
            self.wake_key,
            self.raw_events.disarm_outcomes(),
            recovery,
        )
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    fn prepare_disarms(&mut self, delivered: usize) -> Result<(), Error> {
        self.raw_events.clear_disarms();
        for index in 0..delivered {
            let pending = *self.pending.as_slice().get(index).ok_or(Error::Invariant)?;
            let binding = self.registrations.binding(pending.registration, false)?;
            if !binding.mode.is_one_shot() {
                continue;
            }
            self.raw_events
                .push_disarm(
                    pending.registration,
                    binding.descriptor.as_raw_fd(),
                    binding.interest,
                )
                .ok_or(Error::Invariant)?;
        }
        Ok(())
    }
}
