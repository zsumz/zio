//! Linux epoll syscall parity and caller-arena phase transition regressions.

use std::{
    fs::File,
    io::{self, Write},
    os::{fd::AsFd, unix::net::UnixStream},
};

use crate::{Event, Events, Key, Registration};

use super::epoll::{
    Epoll, EpollBatch, epoll_test_missing_error, epoll_test_permission_error,
    epoll_test_registration_flags,
};

const TEST_TOKEN: u64 = 0xa5a5_1234_fedc_5678;

#[test]
fn unsupported_descriptor_add_reports_eperm() -> Result<(), Box<dyn std::error::Error>> {
    let epoll = Epoll::new()?;
    let source = File::open("/dev/null")?;
    let Err(error) = epoll.add(
        source.as_fd(),
        TEST_TOKEN,
        epoll_test_registration_flags(false),
    ) else {
        return Err(io::Error::other("epoll unexpectedly accepted /dev/null").into());
    };

    assert_eq!(error.raw_os_error(), Some(epoll_test_permission_error()));
    Ok(())
}

#[test]
fn supported_descriptor_add_and_delete_succeed() -> Result<(), Box<dyn std::error::Error>> {
    let epoll = Epoll::new()?;
    let (source, _peer) = UnixStream::pair()?;

    epoll.add(
        source.as_fd(),
        TEST_TOKEN,
        epoll_test_registration_flags(false),
    )?;
    epoll.delete(source.as_fd())?;
    Ok(())
}

#[test]
fn second_delete_reports_enoent() -> Result<(), Box<dyn std::error::Error>> {
    let epoll = Epoll::new()?;
    let (source, _peer) = UnixStream::pair()?;
    epoll.add(
        source.as_fd(),
        TEST_TOKEN,
        epoll_test_registration_flags(false),
    )?;
    epoll.delete(source.as_fd())?;
    let Err(error) = epoll.delete(source.as_fd()) else {
        return Err(io::Error::other("epoll unexpectedly accepted a second delete").into());
    };

    assert_eq!(error.raw_os_error(), Some(epoll_test_missing_error()));
    Ok(())
}

#[test]
fn add_preserves_u64_token_and_one_shot_flag() -> Result<(), Box<dyn std::error::Error>> {
    let epoll = Epoll::new()?;
    let (source, mut peer) = UnixStream::pair()?;
    let flags = epoll_test_registration_flags(true);
    epoll.add(source.as_fd(), TEST_TOKEN, flags)?;
    peer.write_all(&[1])?;

    let mut batch =
        EpollBatch::new(1).ok_or_else(|| io::Error::other("batch construction failed"))?;
    let mut events = Events::with_capacity(1)?;
    let observed = epoll.wait(&mut batch, &mut events, 1_000)?;
    assert_eq!(observed, 1);
    let mut observed_token = None;
    batch.translate(&mut events, observed, None, |token| {
        observed_token = Some(token);
        Ok(Some(classified(token, Key::new(token))))
    })?;

    assert_eq!(observed_token, Some(TEST_TOKEN));
    let [Event::Resource { key, readiness, .. }] = events.as_slice() else {
        return Err(io::Error::other("epoll did not produce one resource event").into());
    };
    assert_eq!(*key, Key::new(TEST_TOKEN));
    assert!(readiness.is_readable());

    events.clear();
    assert_eq!(epoll.wait(&mut batch, &mut events, 0)?, 0);
    epoll.delete(source.as_fd())?;
    Ok(())
}

const fn classified(token: u64, key: Key) -> (Registration, Key) {
    (registration(token), key)
}

const fn registration(token: u64) -> Registration {
    Registration::test(token)
}
