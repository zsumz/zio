//! Level-triggered readiness with the safe duplicate-by-default tier.

use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

use zio::{Interest, Key, Mode, Poll, Wait};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut source, mut peer) = UnixStream::pair()?;
    source.set_nonblocking(true)?;

    let mut poll = Poll::new()?;
    let registration = poll.register(&source, Key::new(1), Interest::READABLE, Mode::Level)?;
    let mut events = poll.events()?;

    peer.write_all(b"a")?;
    let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    let observed = events.iter().any(|event| {
        event.registration() == Some(registration)
            && event.key() == Key::new(1)
            && event.is_readable()
    });
    if !observed {
        return Err(io::Error::other("readiness was not delivered").into());
    }

    let mut byte = [0_u8; 1];
    source.read_exact(&mut byte)?;
    report.into_result()?;
    poll.delete(registration)?;
    Ok(())
}
