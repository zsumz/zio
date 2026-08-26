//! Native kqueue evidence for scoped deletion after state transitions.

use std::{
    io::{self, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
    time::Duration,
};

use crate::{ArmState, Interest, Mode, RegistrationState, sys::MutationFailure};

use super::{
    kqueue::{KeventBatch, Kqueue},
    kqueue_change::{Action, Change, ChangeList, Filter},
    kqueue_policy::{delete_descriptor, exact, modify_descriptor, register_descriptor},
};

const INITIAL_TOKEN: u64 = 101;
const REUSED_TOKEN: u64 = 103;

#[test]
fn native_scoped_delete_leaves_no_filter_after_interest_changes() -> io::Result<()> {
    let combined = Interest::READABLE | Interest::WRITABLE;
    for (prior, desired) in [
        (Interest::READABLE, Interest::WRITABLE),
        (Interest::WRITABLE, Interest::READABLE),
        (combined, Interest::READABLE),
        (Interest::READABLE, combined),
    ] {
        verify_transition(prior, desired)?;
    }
    Ok(())
}

#[test]
fn native_scoped_delete_removes_a_disabled_one_shot_filter() -> io::Result<()> {
    let (source, mut peer) = UnixStream::pair()?;
    let descriptor = source.as_raw_fd();
    let queue = Kqueue::new()?;
    register_descriptor(&queue, descriptor, INITIAL_TOKEN, Interest::READABLE)
        .map_err(MutationFailure::into_source)?;

    let mut changes = ChangeList::new();
    changes
        .push(Change::new(descriptor, Filter::Read, Action::Disable, 0))
        .ok_or_else(|| io::Error::other("one-shot disable overflowed"))?;
    exact(&queue, &changes, false)?;
    delete_descriptor(
        &queue,
        descriptor,
        Interest::READABLE,
        RegistrationState::Registered {
            arm: ArmState::Disarmed,
        },
    )
    .map_err(MutationFailure::into_source)?;

    peer.write_all(&[1])?;
    assert_quiet(&queue)?;
    assert_reusable(&queue, descriptor)
}

fn verify_transition(prior: Interest, desired: Interest) -> io::Result<()> {
    let (source, mut peer) = UnixStream::pair()?;
    let descriptor = source.as_raw_fd();
    let queue = Kqueue::new()?;
    register_descriptor(&queue, descriptor, INITIAL_TOKEN, prior)
        .map_err(MutationFailure::into_source)?;
    modify_descriptor(
        &queue,
        descriptor,
        INITIAL_TOKEN,
        prior,
        Mode::Level,
        ArmState::Armed,
        desired,
        Mode::Level,
    )
    .map_err(MutationFailure::into_source)?;
    delete_descriptor(
        &queue,
        descriptor,
        desired,
        RegistrationState::Registered {
            arm: ArmState::Armed,
        },
    )
    .map_err(MutationFailure::into_source)?;

    peer.write_all(&[1])?;
    assert_quiet(&queue)?;
    assert_reusable(&queue, descriptor)
}

fn assert_quiet(queue: &Kqueue) -> io::Result<()> {
    let mut batch = native_batch()?;
    let observed = queue.wait(&mut batch, Some(Duration::from_millis(10)))?;
    if observed == 0 {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "scoped delete left {observed} native events"
        )))
    }
}

fn assert_reusable(queue: &Kqueue, descriptor: i32) -> io::Result<()> {
    register_descriptor(queue, descriptor, REUSED_TOKEN, Interest::READABLE)
        .map_err(MutationFailure::into_source)?;
    let mut batch = native_batch()?;
    let observed = queue.wait(&mut batch, Some(Duration::from_secs(1)))?;
    let event = batch
        .event(0, observed)
        .ok_or_else(|| io::Error::other("reused descriptor produced no native read event"))?;
    if observed != 1
        || event.ident() != descriptor
        || event.filter() != Filter::Read
        || event.token() != REUSED_TOKEN
    {
        return Err(io::Error::other(format!(
            "unexpected reused descriptor event: {event:?}"
        )));
    }
    delete_descriptor(
        queue,
        descriptor,
        Interest::READABLE,
        RegistrationState::Registered {
            arm: ArmState::Armed,
        },
    )
    .map_err(MutationFailure::into_source)
}

fn native_batch() -> io::Result<KeventBatch> {
    KeventBatch::new(2, 2).ok_or_else(|| io::Error::other("native event storage unavailable"))
}
