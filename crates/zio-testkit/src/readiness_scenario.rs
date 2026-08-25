//! Stable names and parameters for native readiness scenarios.

use zio::{Interest, Mode};

/// Native resource behavior exercised by a readiness scenario.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReadinessFixture {
    /// A Unix stream has buffered bytes followed by a write-half close.
    UnixPendingEof,
    /// A connected Unix stream can accept a write.
    UnixWritable,
    /// A TCP stream has buffered bytes followed by a write-half close.
    TcpPendingEof,
    /// A pipe has buffered bytes followed by its writer closing.
    PipePendingEof,
    /// A pipe writer observes that its reader closed.
    PipeReaderClosed,
}

/// One public-API native readiness behavior covered by the suite.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReadinessScenario {
    /// Unix pending bytes and EOF, read interest, level delivery.
    UnixPendingEofReadableLevel,
    /// Unix pending bytes and EOF, read interest, one-shot delivery.
    UnixPendingEofReadableOneShot,
    /// Unix pending bytes and EOF, combined interest, level delivery.
    UnixPendingEofCombinedLevel,
    /// Unix pending bytes and EOF, combined interest, one-shot delivery.
    UnixPendingEofCombinedOneShot,
    /// Unix writable readiness, level delivery.
    UnixWritableLevel,
    /// Unix writable readiness, one-shot delivery.
    UnixWritableOneShot,
    /// TCP pending bytes and EOF, level delivery.
    TcpPendingEofLevel,
    /// TCP pending bytes and EOF, one-shot delivery.
    TcpPendingEofOneShot,
    /// Pipe pending bytes and EOF, level delivery.
    PipePendingEofLevel,
    /// Pipe pending bytes and EOF, one-shot delivery.
    PipePendingEofOneShot,
    /// A pipe reader closes, level delivery.
    PipeReaderClosedLevel,
    /// A pipe reader closes, one-shot delivery.
    PipeReaderClosedOneShot,
}

impl ReadinessScenario {
    /// Every V1 readiness scenario in stable execution order.
    pub const ALL: [Self; 12] = [
        Self::UnixPendingEofReadableLevel,
        Self::UnixPendingEofReadableOneShot,
        Self::UnixPendingEofCombinedLevel,
        Self::UnixPendingEofCombinedOneShot,
        Self::UnixWritableLevel,
        Self::UnixWritableOneShot,
        Self::TcpPendingEofLevel,
        Self::TcpPendingEofOneShot,
        Self::PipePendingEofLevel,
        Self::PipePendingEofOneShot,
        Self::PipeReaderClosedLevel,
        Self::PipeReaderClosedOneShot,
    ];

    /// Returns the stable scenario name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::UnixPendingEofReadableLevel => "readiness.unix.pending_eof.readable.level",
            Self::UnixPendingEofReadableOneShot => "readiness.unix.pending_eof.readable.one_shot",
            Self::UnixPendingEofCombinedLevel => "readiness.unix.pending_eof.combined.level",
            Self::UnixPendingEofCombinedOneShot => "readiness.unix.pending_eof.combined.one_shot",
            Self::UnixWritableLevel => "readiness.unix.writable.level",
            Self::UnixWritableOneShot => "readiness.unix.writable.one_shot",
            Self::TcpPendingEofLevel => "readiness.tcp.pending_eof.readable.level",
            Self::TcpPendingEofOneShot => "readiness.tcp.pending_eof.readable.one_shot",
            Self::PipePendingEofLevel => "readiness.pipe.pending_eof.readable.level",
            Self::PipePendingEofOneShot => "readiness.pipe.pending_eof.readable.one_shot",
            Self::PipeReaderClosedLevel => "readiness.pipe.reader_closed.writable.level",
            Self::PipeReaderClosedOneShot => "readiness.pipe.reader_closed.writable.one_shot",
        }
    }

    /// Returns the native fixture kind.
    pub const fn fixture(self) -> ReadinessFixture {
        match self {
            Self::UnixPendingEofReadableLevel
            | Self::UnixPendingEofReadableOneShot
            | Self::UnixPendingEofCombinedLevel
            | Self::UnixPendingEofCombinedOneShot => ReadinessFixture::UnixPendingEof,
            Self::UnixWritableLevel | Self::UnixWritableOneShot => ReadinessFixture::UnixWritable,
            Self::TcpPendingEofLevel | Self::TcpPendingEofOneShot => {
                ReadinessFixture::TcpPendingEof
            }
            Self::PipePendingEofLevel | Self::PipePendingEofOneShot => {
                ReadinessFixture::PipePendingEof
            }
            Self::PipeReaderClosedLevel | Self::PipeReaderClosedOneShot => {
                ReadinessFixture::PipeReaderClosed
            }
        }
    }

    /// Returns the registered interest.
    pub const fn interest(self) -> Interest {
        match self {
            Self::UnixPendingEofCombinedLevel | Self::UnixPendingEofCombinedOneShot => {
                Interest::READABLE.union(Interest::WRITABLE)
            }
            Self::UnixWritableLevel
            | Self::UnixWritableOneShot
            | Self::PipeReaderClosedLevel
            | Self::PipeReaderClosedOneShot => Interest::WRITABLE,
            _ => Interest::READABLE,
        }
    }

    /// Returns the delivery mode.
    pub const fn mode(self) -> Mode {
        match self {
            Self::UnixPendingEofReadableOneShot
            | Self::UnixPendingEofCombinedOneShot
            | Self::UnixWritableOneShot
            | Self::TcpPendingEofOneShot
            | Self::PipePendingEofOneShot
            | Self::PipeReaderClosedOneShot => Mode::OneShot,
            _ => Mode::Level,
        }
    }
}
