//! Event-first dispatch followed by post-delivery recovery inspection.

use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

use zio::{Events, Interest, Key, Mode, Poll, Registration, Wait};

fn dispatch_one(
    poll: &mut Poll,
    events: &mut Events,
    source: &mut UnixStream,
    registration: Registration,
) -> Result<(), Box<dyn std::error::Error>> {
    let report = poll.wait(events, Wait::For(Duration::from_secs(1)))?;

    // Delivery and post-delivery recovery can coexist. Consume every event
    // before inspecting or propagating the recovery report.
    if !events
        .iter()
        .any(|event| event.registration() == Some(registration) && event.is_readable())
    {
        return Err(io::Error::other("readiness was not delivered").into());
    }
    let mut byte = [0_u8; 1];
    source.read_exact(&mut byte)?;
    report.into_result()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut source, mut peer) = UnixStream::pair()?;
    source.set_nonblocking(true)?;
    let mut poll = Poll::new()?;
    let registration = poll.register(&source, Key::new(5), Interest::READABLE, Mode::OneShot)?;
    let mut events = poll.events()?;

    peer.write_all(b"a")?;
    dispatch_one(&mut poll, &mut events, &mut source, registration)?;
    poll.delete(registration)?;
    Ok(())
}
