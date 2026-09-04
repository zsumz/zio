//! One-shot delivery and explicit rearming through the public API.

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
    let registration = poll.register(&source, Key::new(2), Interest::READABLE, Mode::OneShot)?;
    let mut events = poll.events()?;

    for (index, byte) in (*b"ab").into_iter().enumerate() {
        peer.write_all(&[byte])?;
        let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
        if !events
            .iter()
            .any(|event| event.registration() == Some(registration) && event.is_readable())
        {
            return Err(io::Error::other("one-shot readiness was not delivered").into());
        }

        let mut received = [0_u8; 1];
        source.read_exact(&mut received)?;
        report.into_result()?;
        if index == 0 {
            poll.rearm(&registration)?;
        }
    }

    poll.delete(registration)?;
    Ok(())
}
