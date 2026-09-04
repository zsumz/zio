# Borrowed registration

Borrowing removes descriptor duplication, but zio cannot express the retained
kernel registration as an ordinary Rust borrow. Keep the numeric descriptor
open and bound to the same open-file description until deletion is proven or
the poller is dropped.

```rust
use std::os::unix::net::UnixStream;
use zio::{Interest, Key, Mode, Poll};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (source, _peer) = UnixStream::pair()?;
    let mut poll = Poll::new()?;

    // SAFETY: `source` remains open with the same descriptor identity until
    // successful deletion, and it has no second borrowed registration here.
    let registration = unsafe {
        poll.register_borrowed(
            &source,
            Key::new(4),
            Interest::READABLE,
            Mode::Level,
        )?
    };

    // Wait and perform nonblocking I/O through `source`.

    poll.delete(registration)?;
    Ok(())
}
```

The obligation also survives copied or dropped handles, disarming, and errors
that return a retained registration. See the full
[borrowed lifecycle tests](../tests/borrowed_registration.rs).
