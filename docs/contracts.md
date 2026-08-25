# Contracts

This page defines zio's portable behavior. Native readiness is advisory; the
matching nonblocking operation is always authoritative.

## Registration ownership

Each successful registration makes its poller retain a distinct descriptor
duplicate and exact generation, even for repeated registration of one source
or its duplicated handles. Caller close and numeric descriptor reuse cannot
redirect later mutations. Handles are poller-scoped; another poller rejects
them. Keys need not be unique.

## Registration lifetime

`Registration` is `Copy`, `Clone`, `Eq`, `Hash`, `Send`, and `Sync`. Every copy
names the same poller and exact generation; copying creates no backend
registration. Dropping any or all copies does not delete it. Retain a copy
outside cancellable work when early cleanup matters.

Successful deletion retires the generation and makes every copy stale. An
`Applied` delete failure does the same; `NotApplied` preserves every copy's
prior state; `Unknown` makes every copy uncertain and allows a delete retry.
Stale and wrong-poller copies are rejected before backend work and cannot affect
a reused slot.

`Applied` and `Unknown` register failures can carry an installed or uncertain
handle. Inspect and retain it before propagating or consuming the error.

## Readiness modes

Level mode reports while a source remains ready. A successful one-shot wait
disarms each delivered registration. Rearming requires an explicit,
successfully applied modification. Recovery failures report the exact state
described below.

## Readiness contract

Readiness is a snapshot, not a promise that later I/O will succeed or avoid
blocking.

| Hint | Meaning |
| --- | --- |
| `READABLE` | A read, receive, or accept may make progress. |
| `WRITABLE` | A write, send, or pending connect may make progress. |
| `READ_CLOSED` | EOF or another terminal readable condition is pending or observable. Buffered data may come first. |
| `WRITE_CLOSED` | The backend reported the writable direction closed or terminally unavailable. |
| `ERROR` | A resource-specific exceptional condition exists. The hint carries no error code. |

Closure hints identify the direction to inspect, not the peer action that
caused them. A native backend may add or omit a closure hint; absence does not
prove that a direction is open. Inspect socket errors with nonblocking I/O and,
when appropriate, `SO_ERROR`. Use the corresponding operation for other
resources. An error or operation result may be the only terminal evidence.

Hints can combine and can appear without a matching requested interest. Test
membership, not equality. For streams, consume positive-length reads until
`WouldBlock`; a zero-length read confirms EOF. Readiness races are normal, so
retry according to the operation result.

One wait emits at most one event per registration and unions split native
hints. Resource events retain first-observation order; a wake follows them.
Separate registrations remain separate even when their keys match.

## Recovery behavior

Kqueue coalesces split filters and submits all delivered one-shot disables in
one receipt-checked batch. Each submitted registration gets an exact outcome:

| Outcome | State |
| --- | --- |
| `Applied` | Disarmed |
| `NotApplied` | Armed |
| `Unknown` | Uncertain |

On recovery failure, `Poll::wait` returns `Error::Recovery` without discarding
translated resource or wake events. The error owns every batch outcome,
including successful peers, after the poller is reused. Other wait errors leave
`Events` empty.

## Allocation contract

Poll construction retains native-event, coalescing, mutation, receipt, and
ownership scratch. Successful waits reuse it without growing zio-owned heap
storage.

`Error::Recovery` alone creates one owned `Vec` snapshot, bounded by the smaller
configured event and registration limit. Allocation exhaustion follows Rust's
normal policy. Formatting, `std::io::Error`, allocator, and operating-system
internals are outside this guarantee.

## Wait behavior

`Wait::NoBlock` and zero duration are nonblocking. Positive durations never
collapse to nonblocking; a backend may round up to its supported resolution,
and scheduling may delay return. Linux rounds up to milliseconds. Kqueue uses
nanosecond fields. Large limits are clamped to the backend integer range, and
interruption may return early.

Use nonblocking descriptors and perform I/O until it would block.

## Wake behavior

A poller binds its wake source to one key. Same-key requests and clones share
it. A conflicting key is rejected without replacing the original.

Triggers may coalesce. One observation drains them, and a later trigger remains
observable. Wake and resource events share the fixed event capacity without
silent loss.

## Mutation outcomes

A failed mutation reports what happened and returns or preserves a handle when
backend state may remain:

| Status | Register | Modify | Delete |
| --- | --- | --- | --- |
| `NotApplied` | Release the reservation; no handle | Preserve prior state | Return the handle in prior state |
| `Applied` | Return registered and armed | Commit and rearm | Return stale after retirement |
| `Unknown` | Return uncertain | Mark uncertain | Return uncertain and retryable |

An uncertain outcome is never presented as a successful rollback.
