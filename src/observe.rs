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
    /// recovery trouble; process every event before reconciling it. Stable
    /// ready sets larger than the batch rotate across calls, even with repeated
    /// wakes.
    pub fn wait(&mut self, events: &mut Events, wait: Wait) -> Result<WaitReport, Error> {
        #[cfg(feature = "unstable-test-support")]
        {
            self.test_wait_metrics = (0, 0, 0);
        }
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
    /// Uses [`Self::wait`] semantics. A reached deadline is nonblocking;
    /// backend interruption remains an [`Error::Io`] failure.
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
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        if self.deferred_wake {
            let key = self.wake_key.ok_or(Error::Invariant)?;
            events
                .try_push(crate::Event::Wake { key })
                .map_err(|_| Error::Invariant)?;
            self.deferred_wake = false;
            return Ok(WaitReport::new(None));
        }
        let observed = self
            .backend
            .wait(&mut self.raw_events, events, wait)
            .map_err(|source| Error::Io {
                operation: Operation::Wait,
                source,
            })?;
        #[cfg(feature = "unstable-test-support")]
        {
            self.test_wait_metrics.0 = observed;
        }
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
        let capacity = self.event_capacity.get();
        let delivery = self.pending.delivery_selection(capacity);
        let deliver_wake = woke && delivery.len() < capacity;
        if woke && self.wake_key.is_none() {
            return Err(Error::Invariant);
        }
        self.deferred_wake = woke;
        let disarm_count = self.prepare_disarms(&delivery)?;
        #[cfg(feature = "unstable-test-support")]
        let disarm_started = (disarm_count != 0).then(Instant::now);
        let recovery = self.backend.submit_disarms(&mut self.raw_events).err();
        #[cfg(feature = "unstable-test-support")]
        {
            self.test_wait_metrics.1 = disarm_count;
            self.test_wait_metrics.2 =
                disarm_started.map_or(0, |started| started.elapsed().as_nanos());
        }
        #[cfg(not(feature = "unstable-test-support"))]
        let _ = disarm_count;
        let pending = delivery.try_iter(self.pending.as_slice())?;
        let result = crate::observe_recovery::finish(
            owner,
            &mut self.registrations,
            events,
            pending,
            delivery.len(),
            deliver_wake,
            self.wake_key,
            self.raw_events.disarm_outcomes(),
            recovery,
        );
        if result.is_ok() && deliver_wake {
            self.deferred_wake = false;
        }
        result
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    fn prepare_disarms(
        &mut self,
        delivery: &crate::pending_kqueue::DeliverySelection,
    ) -> Result<usize, Error> {
        self.raw_events.clear_disarms();
        let mut count = 0_usize;
        for pending in delivery.try_iter(self.pending.as_slice())? {
            let pending = *pending;
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
            count = count.saturating_add(1);
        }
        Ok(count)
    }
}
