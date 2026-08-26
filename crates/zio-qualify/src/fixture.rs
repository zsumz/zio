//! Fresh, independent standard-library fixtures.

use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
};

use crate::{Interest, Scenario};

const PAYLOAD: &[u8] = b"zio-qualify";
const FILL: [u8; 8_192] = [0x5a; 8_192];

#[derive(Debug)]
pub(crate) struct Fixture {
    source: UnixStream,
    peer: UnixStream,
    interest: Interest,
}

impl Fixture {
    pub(crate) fn new(scenario: Scenario) -> io::Result<Self> {
        let (mut source, peer) = UnixStream::pair()?;
        source.set_nonblocking(true)?;
        peer.set_nonblocking(true)?;
        let interest = scenario.interest();
        if interest == Interest::Writable {
            fill_until_blocked(&mut source)?;
        }
        Ok(Self {
            source,
            peer,
            interest,
        })
    }

    pub(crate) fn parts(&mut self) -> (&UnixStream, FixtureDriver<'_>) {
        (
            &self.source,
            FixtureDriver {
                source: &self.source,
                peer: &mut self.peer,
                interest: self.interest,
            },
        )
    }
}

pub(crate) struct FixtureDriver<'fixture> {
    source: &'fixture UnixStream,
    peer: &'fixture mut UnixStream,
    interest: Interest,
}

impl FixtureDriver<'_> {
    pub(crate) fn activate(&mut self) -> io::Result<()> {
        match self.interest {
            Interest::Readable => self.peer.write_all(PAYLOAD),
            Interest::Writable => drain_until_blocked(self.peer),
        }
    }

    pub(crate) fn verify_operation(&mut self) -> io::Result<()> {
        let mut actual = [0_u8; PAYLOAD.len()];
        let mut source = self.source;
        match self.interest {
            Interest::Readable => source.read_exact(&mut actual)?,
            Interest::Writable => {
                source.write_all(PAYLOAD)?;
                self.peer.read_exact(&mut actual)?;
            }
        }
        if actual == PAYLOAD {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "fixture operation returned different bytes",
            ))
        }
    }
}

fn fill_until_blocked(source: &mut UnixStream) -> io::Result<()> {
    loop {
        match source.write(&FILL) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

fn drain_until_blocked(peer: &mut UnixStream) -> io::Result<()> {
    let mut buffer = [0_u8; FILL.len()];
    loop {
        match peer.read(&mut buffer) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}
