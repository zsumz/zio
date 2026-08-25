//! Bounded native waiting and portable event translation.

#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
use std::os::fd::AsRawFd;

use crate::{Error, Event, Events, Mode, Operation, Poll, Wait};

impl Poll {
    /// Replaces `events` with one bounded readiness observation.
    pub fn wait(&mut self, events: &mut Events, wait: Wait) -> Result<(), Error> {
        events.clear();
        self.pending.clear();
        let result = self.observe(events, wait);
        if result.is_err() {
            events.clear();
            self.pending.clear();
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
                self.registrations.mark_disarmed(resource.id)?;
            }
            events
                .try_push(Event::Resource {
                    key: resource.key,
                    readiness: raw.readiness(),
                })
                .map_err(|_| Error::Invariant)?;
        }
        if woke {
            self.backend
                .acknowledge_wake()
                .map_err(|source| Error::Io {
                    operation: Operation::AcknowledgeWake,
                    source,
                })?;
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
        self.disarm_one_shot(delivered)?;
        for pending in self.pending.as_slice().iter().take(delivered) {
            events
                .try_push(Event::Resource {
                    key: pending.key,
                    readiness: pending.readiness,
                })
                .map_err(|_| Error::Invariant)?;
        }
        self.push_wake(events, woke)
    }

    #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "netbsd"))]
    fn disarm_one_shot(&mut self, delivered: usize) -> Result<(), Error> {
        let mut failure = None;
        for pending in self.pending.as_slice().iter().take(delivered) {
            let binding = self.registrations.binding(pending.registration, false)?;
            if binding.mode != Mode::OneShot {
                continue;
            }
            if let Err(observed) = self
                .backend
                .disarm(binding.descriptor.as_raw_fd(), binding.interest)
            {
                failure = Some(observed);
                break;
            }
        }
        if let Some(failure) = failure {
            let affected = self
                .pending
                .as_slice()
                .iter()
                .take(delivered)
                .filter_map(|pending| {
                    self.registrations
                        .binding(pending.registration, true)
                        .ok()
                        .filter(|binding| binding.mode == Mode::OneShot)
                        .map(|_| pending.registration)
                })
                .collect::<Box<[_]>>();
            for registration in &affected {
                self.registrations.mark_uncertain(*registration)?;
            }
            return Err(Error::Recovery(crate::RecoveryFailure::new(
                Operation::Disarm,
                failure.commit(),
                affected,
                failure.into_source(),
            )));
        }
        for pending in self.pending.as_slice().iter().take(delivered) {
            self.registrations.mark_disarmed(pending.registration)?;
        }
        Ok(())
    }

    fn push_wake(&mut self, events: &mut Events, woke: bool) -> Result<(), Error> {
        if let (true, Some(key)) = (woke, self.wake_key) {
            events
                .try_push(Event::Wake { key })
                .map_err(|_| Error::Invariant)?;
        }
        Ok(())
    }
}
