//! Registration ownership and retained-descriptor behavior.

#![cfg(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "freebsd",
    target_os = "netbsd"
))]

mod support;

use std::{io::Write, os::unix::net::UnixStream, thread, time::Duration};

use zio::{CapacityKind, CapacityReason, Error, Event, Interest, Key, Mode, Poll, Wait};

use support::{descriptor_flags, require_no_recovery};

#[test]
fn registration_authority_is_poller_local() -> Result<(), Box<dyn std::error::Error>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut owner = Poll::new()?;
    let mut stranger = Poll::new()?;
    let registration = owner.register(&source, Key::new(7), Interest::READABLE, Mode::Level)?;

    let result = stranger.modify(
        &registration,
        Interest::READABLE | Interest::WRITABLE,
        Mode::Level,
    );
    assert!(matches!(
        result,
        Err(Error::WrongPoller { registration: rejected }) if rejected == registration
    ));

    owner.delete(registration)?;
    Ok(())
}

#[test]
fn poll_retains_the_registered_open_file_description() -> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    let mut poll = Poll::new()?;
    let registration = poll.register(&source, Key::new(11), Interest::READABLE, Mode::Level)?;
    drop(source);

    peer.write_all(b"ready")?;
    let mut events = poll.events()?;
    let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    assert!(
        events
            .iter()
            .any(|event| { matches!(event, Event::Resource { key, .. } if *key == Key::new(11)) })
    );
    require_no_recovery(report)?;

    poll.delete(registration)?;
    Ok(())
}

#[test]
fn retained_duplicate_is_close_on_exec() -> Result<(), Box<dyn std::error::Error>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = Poll::new()?;
    let registration = poll.register(&source, Key::new(17), Interest::READABLE, Mode::Level)?;

    let flags = descriptor_flags(poll.registration_fd(&registration)?)?;
    assert_ne!(flags & libc::FD_CLOEXEC, 0);

    poll.delete(registration)?;
    Ok(())
}

#[test]
fn key_changes_affect_only_future_events() -> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    let mut poll = Poll::new()?;
    let old = Key::new(12);
    let new = Key::new(13);
    let registration = poll.register(&source, old, Interest::READABLE, Mode::Level)?;
    peer.write_all(b"ready")?;

    let mut events = poll.events()?;
    let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    require_no_recovery(report)?;
    assert!(events.iter().any(|event| event.key() == old));

    poll.set_key(&registration, new)?;
    assert!(events.iter().any(|event| event.key() == old));
    assert_eq!(poll.registration_info(&registration)?.key(), new);

    let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    require_no_recovery(report)?;
    assert!(events.iter().any(|event| event.key() == new));

    poll.delete(registration)?;
    assert!(matches!(
        poll.set_key(&registration, old),
        Err(Error::Stale { registration: stale }) if stale == registration.id()
    ));
    Ok(())
}

#[test]
fn registration_capacity_is_fixed() -> Result<(), Box<dyn std::error::Error>> {
    let (first, _first_peer) = UnixStream::pair()?;
    let (second, _second_peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(4, 1)?;
    assert_eq!(poll.event_capacity(), 4);
    assert_eq!(poll.registration_capacity(), 1);
    assert_eq!(poll.registration_count(), 0);
    assert_eq!(poll.remaining_registration_capacity(), 1);
    assert!(poll.is_empty());
    assert!(!poll.is_full());
    let registration = poll.register(&first, Key::new(1), Interest::READABLE, Mode::Level)?;
    assert_eq!(poll.registration_count(), 1);
    assert_eq!(poll.remaining_registration_capacity(), 0);
    assert!(!poll.is_empty());
    assert!(poll.is_full());
    assert_eq!(poll.registrations()?, vec![registration]);

    let result = poll.register(&second, Key::new(2), Interest::READABLE, Mode::Level);
    assert!(matches!(
        result,
        Err(error)
            if matches!(
                error.error(),
                Error::Capacity {
                    kind: CapacityKind::Registration,
                    limit: 1,
                    reason: CapacityReason::Exhausted,
                    ..
                }
            )
    ));
    assert_eq!(poll.registration_count(), 1);
    assert!(poll.is_full());

    poll.delete(registration)?;
    assert_eq!(poll.registration_count(), 0);
    assert_eq!(poll.remaining_registration_capacity(), 1);
    assert!(!poll.is_full());
    assert!(poll.registrations()?.is_empty());
    Ok(())
}

#[test]
fn retained_registrations_iterate_in_place() -> Result<(), Box<dyn std::error::Error>> {
    let (first_source, _first_peer) = UnixStream::pair()?;
    let (second_source, _second_peer) = UnixStream::pair()?;
    let (third_source, _third_peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(3, 3)?;
    assert_eq!(poll.iter_registrations()?.len(), 0);
    let first = poll.register(&first_source, Key::new(14), Interest::READABLE, Mode::Level)?;
    let second = poll.register(
        &second_source,
        Key::new(15),
        Interest::WRITABLE,
        Mode::OneShot,
    )?;
    let third = poll.register(&third_source, Key::new(16), Interest::READABLE, Mode::Level)?;
    poll.delete(second)?;

    let mut registrations = poll.iter_registrations()?;
    assert_eq!(registrations.len(), 2);
    assert_eq!(registrations.size_hint(), (2, Some(2)));
    let front = registrations.next().ok_or("missing front registration")?;
    let back = registrations
        .next_back()
        .ok_or("missing back registration")?;
    assert_ne!(front, back);
    assert!([first, third].contains(&front));
    assert!([first, third].contains(&back));
    assert_eq!(registrations.len(), 0);
    assert!(registrations.next().is_none());
    assert!(registrations.next_back().is_none());
    drop(registrations);

    poll.delete_all()?;
    Ok(())
}

#[test]
fn delete_all_retires_every_registration() -> Result<(), Box<dyn std::error::Error>> {
    let (first, _first_peer) = UnixStream::pair()?;
    let (second, _second_peer) = UnixStream::pair()?;
    let mut poll = Poll::with_capacity(2, 2)?;
    let first = poll.register(&first, Key::new(1), Interest::READABLE, Mode::Level)?;
    let second = poll.register(&second, Key::new(2), Interest::WRITABLE, Mode::OneShot)?;

    poll.delete_all()?;

    assert_eq!(poll.registration_count(), 0);
    for registration in [first, second] {
        assert!(matches!(
            poll.registration_state(&registration),
            Err(Error::Stale { registration: stale }) if stale == registration.id()
        ));
    }
    Ok(())
}

#[test]
fn poll_can_move_to_its_owning_thread() -> Result<(), Box<dyn std::error::Error>> {
    let (source, _peer) = UnixStream::pair()?;
    let poll = Poll::with_capacity(1, 1)?;
    let remaining = thread::spawn(move || {
        let mut poll = poll;
        let registration = poll
            .register(&source, Key::new(3), Interest::READABLE, Mode::Level)
            .map_err(|error| error.to_string())?;
        poll.delete(registration)
            .map_err(|error| error.to_string())?;
        Ok::<_, String>(poll.remaining_registration_capacity())
    })
    .join()
    .map_err(|_| "poll thread panicked")??;

    assert_eq!(remaining, 1);
    Ok(())
}
