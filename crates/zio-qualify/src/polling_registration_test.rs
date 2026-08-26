//! Safety-contract proofs for the sole `polling` registration leaf.

use std::{io, os::unix::net::UnixStream, sync::Arc};

use polling::{Event, PollMode, Poller};

use crate::polling_registration::PollingRegistration;

#[test]
fn failed_delete_drops_exclusive_poller_while_source_lives() -> Result<(), io::Error> {
    let (source, peer) = UnixStream::pair()?;
    let poller = Arc::new(Poller::new()?);
    let weak_poller = Arc::downgrade(&poller);
    let registration =
        PollingRegistration::shared(poller, &source, Event::readable(41), PollMode::Oneshot)?;
    registration.poller().delete(registration.source())?;
    let delete_result = registration.delete();
    check(
        delete_result.is_err(),
        "second delete unexpectedly succeeded; failure path was not exercised",
    )?;
    check(
        weak_poller.upgrade().is_none(),
        "exclusive poller outlived the failed registration delete",
    )?;
    check(
        source.peer_addr().is_ok() && peer.peer_addr().is_ok(),
        "source pair did not outlive the failed delete",
    )
}

fn check(condition: bool, message: &'static str) -> Result<(), io::Error> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message))
    }
}
