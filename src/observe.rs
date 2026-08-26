//! Bounded native waiting and portable event translation.

#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
use std::os::fd::AsRawFd;

use crate::{Error, Events, Mode, Operation, Poll, Wait};

impl Poll {
    /// Replaces `events` with one bounded readiness observation.
    ///
    /// The destination is cleared on entry. Every error except
    /// [`Error::Recovery`] leaves it empty. A recovery error retains every
    /// resource and wake event translated before one-shot recovery failed.
    pub fn wait(&mut self, events: &mut Events, wait: Wait) -> Result<(), Error> {
        events.clear();
        self.pending.clear();
        let result = self.observe(events, wait);
        self.pending.clear();
        if result.is_err() && !matches!(&result, Err(Error::Recovery(_))) {
            events.clear();
        }
        result
    }

    fn observe(&mut self, events: &mut Events, wait: Wait) -> Result<(), Error> {
        if events.capacity() < self.event_capacity.get() {
            return Err(Error::EventsTooSmall {
                required: self.event_capacity.get(),
                actual: events.capacity(),
            });
        }
        let observed = self
            .backend
            .wait(&mut self.raw_events, wait)
            .map_err(|source| Error::Io {
                operation: Operation::Wait,
                source,
            })?;
        #[cfg(target_os = "linux")]
        self.translate_linux(observed, events)?;
        #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
        self.translate_kqueue(observed, events)?;
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "freebsd",
            target_os = "netbsd"
        )))]
        let _ = observed;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn translate_linux(&mut self, observed: usize, events: &mut Events) -> Result<(), Error> {
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
            if raw.descriptor() >= 0 && raw.descriptor() != resource.descriptor {
                continue;
            }
            if resource.mode == Mode::OneShot {
                self.registrations
                    .apply_disarm(resource.id, crate::CommitStatus::Applied)?;
            }
            events
                .try_push(crate::Event::Resource {
                    key: resource.key,
                    readiness: raw.readiness(),
                })
                .map_err(|_| Error::Invariant)?;
        }
        self.push_wake(events, woke)
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    fn translate_kqueue(&mut self, observed: usize, events: &mut Events) -> Result<(), Error> {
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
            if binding.mode != Mode::OneShot {
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

    #[cfg(target_os = "linux")]
    fn push_wake(&mut self, events: &mut Events, woke: bool) -> Result<(), Error> {
        if let (true, Some(key)) = (woke, self.wake_key) {
            events
                .try_push(crate::Event::Wake { key })
                .map_err(|_| Error::Invariant)?;
        }
        Ok(())
    }
}
