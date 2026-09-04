//! Zero-duplication ownership transfer and explicit descriptor recovery.

use std::{
    io::{self, Read, Write},
    os::{
        fd::{BorrowedFd, OwnedFd},
        unix::net::UnixStream,
    },
    time::Duration,
};

use zio::{Interest, Key, Mode, Poll, Wait};

fn read_byte(descriptor: BorrowedFd<'_>) -> Result<u8, io::Error> {
    // `UnixStream` needs ownership, so duplicate only for this typed I/O view.
    let mut source = UnixStream::from(descriptor.try_clone_to_owned()?);
    let mut byte = [0_u8; 1];
    source.read_exact(&mut byte)?;
    Ok(byte[0])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (source, mut peer) = UnixStream::pair()?;
    source.set_nonblocking(true)?;
    let descriptor: OwnedFd = source.into();

    let mut poll = Poll::new()?;
    let registration =
        poll.register_owned(descriptor, Key::new(3), Interest::READABLE, Mode::Level)?;
    let mut events = poll.events()?;

    peer.write_all(b"a")?;
    let report = poll.wait(&mut events, Wait::For(Duration::from_secs(1)))?;
    if !events
        .iter()
        .any(|event| event.registration() == Some(registration) && event.is_readable())
    {
        return Err(io::Error::other("owned readiness was not delivered").into());
    }
    let _byte = read_byte(poll.registration_fd(&registration)?)?;
    report.into_result()?;

    let descriptor = poll.delete_owned(registration)?;
    let source = UnixStream::from(descriptor);
    drop(source);
    Ok(())
}
