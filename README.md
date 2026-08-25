<p align="center">
  <img src="./zio-logo.svg" alt="zio" width="720">
</p>

<p align="center">
  <strong>Bounded, explicit readiness I/O for Rust.</strong>
</p>

<p align="center">
  zio is a small, synchronous readiness poller built directly on epoll and
  kqueue, without becoming an async runtime.
</p>

<p align="center">
  <a href="#model">Model</a>
  <span> · </span>
  <a href="#crates">Crates</a>
  <span> · </span>
  <a href="#start">Start</a>
  <span> · </span>
  <a href="#qualification">Qualification</a>
</p>

<br />

## Model

```text
Linux                          epoll + eventfd
64-bit macOS, FreeBSD, NetBSD  kqueue + EVFILT_USER
```

Callers choose registration and event limits, readiness interests, delivery
modes, event keys, and blocking behavior. Ordinary waits reuse fixed storage.

### Registration ownership

A registration is a move-only capability owned by one poller; another poller
cannot use it. Keys need not be unique. Every successful registration,
including repeated calls for the same source or its duplicated handles, owns a
distinct descriptor duplicate and exact generation. Caller close and numeric
descriptor reuse therefore cannot redirect later mutations.
`Poll::delete` releases it early; dropping the capability alone leaves the
registration retained until the poller is dropped.

### Readiness modes

Level mode reports while a source remains ready. One-shot mode disarms after
delivery and requires an explicit modification whose backend mutation is
applied.

### Readiness contract

Readiness is an advisory snapshot, not a promise that a later operation will
succeed or avoid blocking. `READABLE` means a read, receive, or accept may make
progress. `WRITABLE` means a write, send, or pending connect may make progress.
The operation itself remains the source of truth.

Closure is directional. `READ_CLOSED` means EOF or another terminal condition
is pending or observable on the readable direction. Buffered data may still be
returned first, so it can be combined with `READABLE`: consume positive-length
reads before a zero-length stream read confirms EOF. `WRITE_CLOSED` means the
backend reported the writable direction closed or terminally unavailable. The
closure hints identify which operation direction to inspect, not the peer
action that caused the condition. Native reporting can conservatively include
an additional closure hint or omit one; `ERROR` or the operation result may be
the only terminal evidence. The two hints may appear together or independently.

`ERROR` reports a resource-specific exceptional condition but contains no error
code. Inspect a socket with its nonblocking operation and, where appropriate,
`SO_ERROR`; inspect other resources with their corresponding nonblocking I/O.
Close and error reporting differs by resource and platform, so treat these
hints as best-effort evidence and use the operation result as final state.

Hints may combine, and close or error hints may appear even when the matching
direction was not requested. Test membership rather than comparing a readiness
set for equality. For streams, repeatedly consume positive-length nonblocking
reads, stop on `WouldBlock`, and treat zero as confirmed EOF. A readiness race
is normal: retry only according to the operation's result.

One wait emits at most one resource event per registration, unioning hints from
split native observations. Resource events retain first-native-observation
order; a wake follows them. Separate registrations remain separate even when
their caller keys match. On a successful one-shot wait, any delivered resource
event disarms that exact registration. A recovery error can preserve an event
while its exact outcome leaves the registration disarmed, armed, or uncertain,
as described below.

### Recovery behavior

Kqueue coalesces split filters and submits every delivered one-shot disable in
one receipt-checked batch. Each submitted registration receives an exact
outcome: `Applied` and disarmed, `NotApplied` and armed, or `Unknown` and
uncertain. If recovery fails, `Poll::wait` returns `Error::Recovery` without
discarding resource or wake events already translated. The error owns every
batch outcome, including successful peers, so its evidence remains exact after
the poller is reused. Other wait errors leave `Events` empty.

### Allocation contract

Poll construction retains native-event, coalescing, change, receipt, and
ownership scratch. Successful waits reuse it without growing zio-owned heap
storage. `Error::Recovery` alone creates one owned `Vec` snapshot containing at
most the smaller configured event and registration limits. Allocation
exhaustion follows Rust's ordinary allocation-error policy; formatting,
`std::io::Error`, allocator, and operating-system internals are outside this
storage guarantee.

### Wait behavior

`Wait::NoBlock` and a zero-duration wait are nonblocking. A positive duration
never collapses to nonblocking; a backend may round it up to its supported
timeout resolution, and scheduling may delay return. Linux currently rounds up
to whole milliseconds; kqueue supplies nanosecond fields. Very large limits
are clamped to the backend integer range, and interruption may return early.
Readiness is advisory, so callers use nonblocking descriptors and perform I/O
until it would block.

### Wake behavior

A poller binds its wake source to one caller key. Same-key requests and clones
share it; a conflicting key is rejected without replacing the original.
Repeated triggers may coalesce, but one observation fully drains them and a
later trigger remains observable. Wake and resource events share the fixed
event capacity without being silently lost.

### Mutation outcomes

A failed mutation exposes what happened and returns or preserves a capability
when backend state may remain:

| Status | Register | Modify | Delete |
| --- | --- | --- | --- |
| `NotApplied` | release the reservation; return no capability | preserve the prior state | return the capability in its prior state |
| `Applied` | return a registered, armed capability | commit and rearm | return a stale capability after retirement |
| `Unknown` | return an uncertain capability | mark uncertain | return an uncertain, retryable capability |

An uncertain outcome is never silently presented as a successful rollback.

Unsafe code is confined to the epoll, eventfd, and kqueue syscall leaves. zio
does not provide edge triggering, Windows support, timers, signals, process
watching, socket construction, an executor, or an async runtime.

## Crates

| Crate | Purpose |
| --- | --- |
| `zio` | Synchronous readiness polling, ownership, and native backends |
| `zio-testkit` | Workspace-private mutation, wake, and readiness conformance |

The testkit drives the portable mutation reducer through an opt-in support
feature and exercises wake and readiness behavior through zio's ordinary public
API. Its native reports use stable scenario names and structured failures.
Normal builds contain no testkit dependency, and its public vocabulary exposes
no raw descriptors, syscall structures, or native backend trait.

## Start

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
    poll.wait(&mut events, Wait::NoBlock)?;
    poll.delete(registration)?;
    Ok(())
}
```

## Qualification

```sh
zcheck run check
zrail diff --base HEAD --deny-grants
```

`zcheck` is the complete local gate for source shape, zrail architecture,
formatting, Clippy, rustdoc, MSRV and current-toolchain tests, doctests, package
contents, and diff hygiene. The zrail diff separately reviews changes to
architectural authority. Use zcheck 0.0.2 and zrail 0.0.3-rc.4, matching CI and
the reviewed lock.

CI runs native Linux and macOS backend tests and cross-compiles FreeBSD and
NetBSD. The BSD backends remain compile-qualified experimental support until
native execution is added. zio supports Rust 1.88 and newer. zio is a published
pre-alpha and is not release-ready yet.

## License

Apache-2.0. See [LICENSE](LICENSE).
