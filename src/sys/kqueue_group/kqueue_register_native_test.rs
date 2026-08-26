//! Native proof that receipt-free registration does not drain readiness.

use std::{
    io::{self, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
    time::Duration,
};

use crate::{Interest, sys::MutationFailure};

use super::{
    kqueue::{KeventBatch, Kqueue},
    kqueue_change::Filter,
    kqueue_policy::register_descriptor,
};

const FIRST_TOKEN: u64 = 151;
const SECOND_TOKEN: u64 = 157;

#[test]
fn registering_a_second_ready_descriptor_does_not_drain_pending_readiness() -> io::Result<()> {
    let (first, mut first_peer) = UnixStream::pair()?;
    let (second, mut second_peer) = UnixStream::pair()?;
    let queue = Kqueue::new()?;

    register_descriptor(&queue, first.as_raw_fd(), FIRST_TOKEN, Interest::READABLE)
        .map_err(MutationFailure::into_source)?;
    first_peer.write_all(&[1])?;
    second_peer.write_all(&[2])?;
    register_descriptor(&queue, second.as_raw_fd(), SECOND_TOKEN, Interest::READABLE)
        .map_err(MutationFailure::into_source)?;

    let mut batch = KeventBatch::new(2, 2)
        .ok_or_else(|| io::Error::other("native event storage unavailable"))?;
    let observed = queue.wait(&mut batch, Some(Duration::from_secs(1)))?;
    let mut first_seen = false;
    let mut second_seen = false;
    for index in 0..observed {
        let event = batch
            .event(index, observed)
            .ok_or_else(|| io::Error::other("missing native registration event"))?;
        if event.filter() != Filter::Read {
            return Err(io::Error::other(format!(
                "unexpected native registration event: {event:?}"
            )));
        }
        first_seen |= event.token() == FIRST_TOKEN && event.ident() == first.as_raw_fd();
        second_seen |= event.token() == SECOND_TOKEN && event.ident() == second.as_raw_fd();
    }
    if observed == 2 && first_seen && second_seen {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "expected both pending descriptors, observed={observed}, first={first_seen}, second={second_seen}"
        )))
    }
}
