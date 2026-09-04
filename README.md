<p align="center">
  <img src="./zio-logo.svg" alt="zio" width="720">
</p>

<p align="center">
  <strong>Bounded, explicit readiness I/O for Rust.</strong>
</p>

zio is a small synchronous poller built directly on epoll and kqueue. It is not
an async runtime.

## Why zio?

zio owns descriptor identity, rejects stale generations, bounds retained
storage, and reports whether failed kernel mutations were applied. Use it when
those ownership and recovery guarantees matter more than broad portability.

Use Mio when you need Windows, WASI, provided networking types, or its wider
ecosystem. Use `polling` when you need a broader backend set or edge delivery.

## Support

| Platform | Backend | Status |
| --- | --- | --- |
| Linux | epoll + eventfd | Native-qualified |
| 64-bit macOS | kqueue + `EVFILT_USER` | Native-qualified |
| 64-bit FreeBSD, NetBSD | kqueue + `EVFILT_USER` | Native CI, experimental |

`Poll::has_native_backend()` reports availability without constructing a poller.

Requires Rust 1.88 or newer.

## Use

```rust
use std::net::TcpListener;
use zio::{Interest, Key, Mode, Poll, Wait};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;

    let mut poll = Poll::new()?;
    let registration =
        poll.register(&listener, Key::new(7), Interest::READABLE, Mode::Level)?;

    let mut events = poll.events()?;
    let report = poll.wait(&mut events, Wait::NoBlock)?;
    for event in &events {
        println!("{event:?}");
    }
    report.into_result()?;
    poll.delete(registration)?;
    Ok(())
}
```

Callers choose registration and event limits, interests, delivery modes, keys,
and wait behavior. Ordinary waits reuse fixed zio-owned storage. A recovery
report may allocate one bounded snapshot.

Use duplicate-by-default `register` when integrating a caller-owned descriptor.
High-cardinality reactors that transfer descriptor ownership should prefer
`register_owned`, which avoids the duplicate and returns the descriptor through
`delete_owned`. See the [owned example](examples/owned.rs).

For non-default limits, use named construction:

```rust
# use zio::Poll;
# fn main() -> Result<(), zio::Error> {
let poll = Poll::builder()
    .event_capacity(1_024)
    .registration_capacity(65_536)
    .build()?;
# drop(poll);
# Ok(())
}
```

## Contracts

- Pollers duplicate descriptors by default.
- Owned transfer returns descriptors on rejection or explicit owned deletion.
- Unsafe borrowing skips duplication and remains caller-owned.
- Level delivery repeats; one-shot delivery requires explicit rearming.
- Readiness is advisory. Nonblocking I/O remains the source of truth.
- Pollers are `Send`, not `Sync`; wakers are `Send + Sync`.
- Wake triggers may coalesce. One observation consumes the logical pending
  notification, and a later successful trigger remains observable.
- Mutation and recovery failures report whether backend state changed.

See [Contracts](docs/contracts.md) for the precise guarantees.

## Examples

- [Level readiness](examples/level.rs)
- [One-shot rearming](examples/one_shot.rs)
- [Owned descriptor transfer](examples/owned.rs)
- [Unsafe borrowed registration](examples/borrowed.md)
- [Recovery-aware dispatch](examples/recovery.rs)
- [Nested pollers](examples/nested.md)

## Crates

| Crate | Purpose |
| --- | --- |
| [`zio`](https://docs.rs/zio) | Readiness polling, ownership, and native backends |
| [`zio-testkit`](https://github.com/zsumz/zio/blob/main/crates/zio-testkit/README.md) | Workspace-private mutation, wake, and readiness conformance |

## Verify

```sh
zcheck run check
zrail diff --base HEAD --deny-grants
```

See [Qualification](docs/qualification.md) for the evidence model and
[Performance](docs/performance.md) for reproducible peer measurements.

The [API and MSRV policies](docs/contracts.md#api-evolution),
[changelog](CHANGELOG.md), [contribution guide](CONTRIBUTING.md), and
[security policy](SECURITY.md) describe the release boundary.

## Scope

zio does not provide edge triggering, Windows support, timers, signals,
process watching, socket construction, an executor, or an async runtime. Unsafe
code is confined to reviewed syscall and borrowed-descriptor leaves.

zio is a [published pre-alpha](https://crates.io/crates/zio) and is not release-ready.

## License

Apache-2.0. See [LICENSE](LICENSE).
