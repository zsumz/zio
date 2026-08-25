# zio

**Bounded, explicit readiness I/O for Rust.**

`zio` is a small, synchronous readiness poller built directly on epoll and
kqueue. It makes registration ownership, event capacity, wake behavior, and
uncertain operating-system mutations explicit without becoming an async
runtime.

The crate is under active development and is not release-ready yet.

## Start

```rust
use std::{net::TcpListener, time::Duration};
use zio::{Event, Interest, Key, Mode, Poll, Wait};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;

    let mut poll = Poll::new()?;
    let registration = poll.register(
        &listener,
        Key::new(7),
        Interest::READABLE,
        Mode::Level,
    )?;

    let mut events = poll.events()?;
    poll.wait(&mut events, Wait::For(Duration::from_millis(100)))?;
    for event in &events {
        if let Event::Resource { key, readiness } = event {
            println!("{key:?}: {readiness:?}");
        }
    }

    poll.delete(registration)?;
    Ok(())
}
```

## Scope

The initial implementation targets:

- Linux through epoll and eventfd.
- 64-bit macOS, FreeBSD, and NetBSD through kqueue and `EVFILT_USER`.
- Level-triggered and one-shot descriptor readiness.
- Caller-selected event keys and bounded, allocation-stable event batches.
- Move-only registrations owned by exactly one poller.
- Mutation failures classified as applied, not applied, or unknown.

Duplicate keys are valid. Registering the same descriptor twice is rejected,
including while its earlier registration remains retained for recovery.

## Registration ownership

A registration is a move-only capability owned by one poller. The poller rejects
duplicate descriptors and handles from another poller, while caller-selected
keys may be shared intentionally. The poller retains its own duplicate of each
descriptor, so caller close and descriptor-number reuse cannot redirect later
mutations. Pass the capability to `Poll::delete` for early release; dropping the
capability alone leaves the registration retained until the poller is dropped.

## Readiness modes

Level mode continues reporting readiness while the source remains ready.
One-shot mode disarms after delivery and requires an explicit successful rearm.

## Mutation outcomes

A failed registration mutation reports whether the requested kernel change was
applied, was not applied, or cannot be proven. An uncertain outcome is never
silently presented as a successful rollback.

Ordinary waits reuse fixed storage. Constructing a wait-time recovery failure
currently allocates its bounded affected-registration list; removing that
recovery-only allocation remains pre-release work.

Windows, edge-triggered mode, timers, signals, process watching, socket
construction, executors, and async-runtime integration are intentionally out of
scope for the first release.

## Safety

Unsafe code is limited to the epoll, eventfd, and kqueue syscall leaves. Each
leaf documents the operating-system contract that makes its pointer, lifetime,
and descriptor assumptions valid. Portable state and policy remain safe Rust.

## Minimum supported Rust version

`zio` supports Rust 1.88 and newer. The canonical qualification graph runs the
MSRV directly in addition to the repository-pinned toolchain.

## Qualification

```sh
zcheck run check
zrail diff --base HEAD --deny-grants
```

The graph covers repository policy, formatting, Clippy, rustdoc, MSRV and
current-toolchain tests, doctests, and the publishable crate archive. CI runs
the native Linux and macOS backends and cross-compiles FreeBSD and NetBSD on
both supported compiler lanes.

## License

Apache-2.0. See [LICENSE](LICENSE).
