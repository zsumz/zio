//! Borrowed benchmark adapter lifetime and terminal-invalidation regressions.

use std::{io::Write, os::unix::net::UnixStream, time::Duration};

use super::{
    backend::{Backend, Profile, display},
    zio_borrowed_backend::{ZioBorrowedBackend, ZioBorrowedRegistration},
};

#[test]
fn source_lifetime_marker_adds_no_storage() {
    assert_eq!(
        std::mem::size_of::<ZioBorrowedRegistration<'static>>(),
        std::mem::size_of::<zio::Registration>()
    );
}

#[test]
fn optional_poller_adds_no_storage() {
    assert_eq!(
        std::mem::size_of::<Option<zio::Poll>>(),
        std::mem::size_of::<zio::Poll>()
    );
}

#[test]
fn register_failure_drops_poller_with_live_registration() -> Result<(), String> {
    let (first, _first_peer) = UnixStream::pair().map_err(display)?;
    let (second, _second_peer) = UnixStream::pair().map_err(display)?;
    let mut backend = ZioBorrowedBackend::new(1, 1)?;
    let registration = backend.register(&first, 1, Profile::Level)?;

    let error = backend
        .register(&second, 2, Profile::Level)
        .err()
        .ok_or_else(|| "capacity registration unexpectedly succeeded".to_owned())?;
    assert!(error.contains("capacity"));
    assert!(backend.poll_mut().is_err());
    std::hint::black_box(&registration);
    Ok(())
}

#[test]
fn rearm_failure_drops_poller_before_guard() -> Result<(), String> {
    let (source, _peer) = UnixStream::pair().map_err(display)?;
    let mut backend = ZioBorrowedBackend::new(1, 1)?;
    let registration = backend.register(&source, 1, Profile::Level)?;
    backend
        .poll_mut()?
        .delete(registration.registration)
        .map_err(display)?;

    assert!(backend.rearm(&registration, Profile::Level).is_err());
    assert!(backend.poll_mut().is_err());
    std::hint::black_box(&registration);
    Ok(())
}

#[test]
fn delete_failure_drops_poller_before_consumed_guard() -> Result<(), String> {
    let (source, _peer) = UnixStream::pair().map_err(display)?;
    let mut backend = ZioBorrowedBackend::new(1, 1)?;
    let registration = backend.register(&source, 1, Profile::Level)?;
    backend
        .poll_mut()?
        .delete(registration.registration)
        .map_err(display)?;

    assert!(backend.delete(registration).is_err());
    assert!(backend.poll_mut().is_err());
    Ok(())
}

#[test]
fn observation_failure_drops_poller_before_guard() -> Result<(), String> {
    let (source, mut peer) = UnixStream::pair().map_err(display)?;
    let mut backend = ZioBorrowedBackend::new(1, 1)?;
    let registration = backend.register(&source, 1, Profile::Level)?;
    peer.write_all(&[1]).map_err(display)?;

    let error = backend
        .wait(Duration::from_secs(1), &mut |_| {
            Err("injected observer failure".to_owned())
        })
        .err()
        .ok_or_else(|| "observer failure unexpectedly succeeded".to_owned())?;
    assert_eq!(error, "injected observer failure");
    assert!(backend.poll_mut().is_err());
    std::hint::black_box(&registration);
    Ok(())
}
