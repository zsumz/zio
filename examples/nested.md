# Nested pollers

`Poll` exposes its selector descriptor as a trusted composition escape hatch,
so an outer poller can observe when an inner poller has work. Do not wait on or
mutate the selector through another API.

```rust
use std::time::Duration;
use zio::{Interest, Key, Mode, Poll, Wait};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut inner = Poll::new()?;
    let mut outer = Poll::new()?;
    let nested = outer.register(&inner, Key::new(6), Interest::READABLE, Mode::Level)?;
    let waker = inner.waker(Key::new(7))?;

    waker.wake()?;
    let mut outer_events = outer.events()?;
    outer
        .wait(&mut outer_events, Wait::For(Duration::from_secs(1)))?
        .into_result()?;

    let mut inner_events = inner.events()?;
    inner.wait(&mut inner_events, Wait::NoBlock)?.into_result()?;
    outer.delete(nested)?;
    Ok(())
}
```

Draining the inner poller clears its readiness in the outer poller; later inner
work makes it readable again. The
[composition test](../tests/poller_descriptor.rs) exercises that full cycle.
