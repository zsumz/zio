# Contracts

This page defines zio's portable behavior. Native readiness is advisory; the
matching nonblocking operation is always authoritative.

## API evolution

`Error` and `Operation` are open diagnostic vocabularies. Downstream matches
must include a fallback arm. `Event`, `CommitStatus`, `DescriptorOwnership`,
`Mode`, `Wait`, `ArmState`, and `RegistrationState` are intentionally closed
domains; changing their cases is a breaking contract change. Event fields may
grow; match with `..`.

Use `Error` accessors for operation, commit status, and typed I/O details.

## Registration ownership

`Poll::register` is the safe default. Each successful call retains a distinct
descriptor duplicate and exact generation, even for repeated registration of
one source or its duplicated handles. Caller close and numeric descriptor reuse
cannot redirect later mutations. Handles are poller-scoped; another poller
rejects them. Keys need not be unique. Each resource event carries its exact
registration handle.

`Poll::register_owned` transfers an `OwnedFd` without duplication. A
handle-bearing failure retains it; every other failure closes it.
`Poll::registration_fd` safely borrows any retained resource descriptor,
including one in uncertain backend state.
`Poll::registrations` returns a bounded owned snapshot for audit or cleanup.
`RegistrationInfo::descriptor_ownership` reports who owns the retained descriptor.
Remaining capacity excludes live and generation-exhausted slots.

`Poll::set_key` changes only future resource-event routing and does no backend work.
`RegistrationState::arm` returns `None` when backend state is uncertain.

## Borrowed registration

`Poll::register_borrowed` skips the duplicate and is unsafe. The exact numeric
descriptor must stay open and identify the same open-file description until
deletion retires the registration or the poller is dropped. Do not concurrently
close or reassign it. The same descriptor may not be borrowed into one poller
twice at the same time; use a real duplicate for a second registration.

The obligation continues while a registration is disarmed or uncertain, after
dropping any handle copies, and after register or delete failures that retain a
live registration. It ends after successful deletion or an `Applied` delete
failure. `NotApplied` and `Unknown` deletion failures retain it.
`RegisterError::registration() == None` ends it when the call returns.

Explicit deletion gives deterministic native reclamation. On kqueue, a waker
can outlive its poller and retain the queue; closing the source still removes
any residual knotes.

## Registration lifetime

`Registration` is `Copy`, `Clone`, `Eq`, `Ord`, `Hash`, `Send`, and `Sync`.
Every copy names the same poller and exact generation; copying creates no
backend registration. Dropping any or all copies does not delete it. Retain a
copy outside cancellable work when early cleanup matters.

Ordering supports ordered containers; it does not express registration age.

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
successfully applied mutation. `Poll::rearm` preserves interest and mode;
`Poll::modify` replaces them. Recovery failures report the exact state below.

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

Direct readiness predicates return `false` for wake events.

One wait emits at most one event per registration and unions split native
hints. Resource events retain first-observation order; a wake follows them.
Separate registrations remain separate even when their keys match.

## Recovery behavior

Kqueue coalesces split filters and submits all delivered one-shot disables in
one receipt-checked batch. A filter proven already absent satisfies the disarm
postcondition. Each submitted registration gets an exact outcome:

| Outcome | State |
| --- | --- |
| `Applied` | Disarmed |
| `NotApplied` | Armed |
| `Unknown` | Uncertain |

Each outcome carries the exact registration handle.

`Poll::wait` returns `Ok(WaitReport)` after valid delivery. Process the retained
resource and wake events first, then inspect `WaitReport::recovery`. A recovery
failure owns every batch outcome, including successful peers, after the poller
is reused. Returning `Err` means delivery failed and leaves `Events` empty.

## Allocation contract

Poll construction retains native-event, coalescing, mutation, receipt, and
ownership scratch. Successful waits reuse it without growing zio-owned heap
storage. A configured wake trigger and observation also allocate nothing.

Kqueue retains observation space for both filters of every registration plus
the wake filter so a delivered registration receives its complete split-filter
snapshot. One-shot recovery plans and receipts are bounded separately by the
smaller configured event and registration limit.

A failed post-delivery recovery alone creates one owned `Vec` snapshot, bounded
by the smaller configured event and registration limit. Allocation exhaustion
follows Rust's normal policy. Formatting, `std::io::Error`, allocator, and
operating-system internals are outside this guarantee.

## Wait behavior

`Wait::NoBlock` and zero duration are nonblocking. Positive durations never
collapse to nonblocking; a backend may round up to its supported resolution,
and scheduling may delay return. Linux rounds up to milliseconds. Kqueue uses
nanosecond fields. Large limits are clamped to the backend integer range, and
interruption may return early.

Use nonblocking descriptors and perform I/O until it would block.

## Wake behavior

A poller binds its wake source to one key. Same-key requests and clones share
it, and each `Waker` reports that key. A conflicting key is rejected without
replacing the original. `Poll::waker_key` reports the current binding.

Triggers may coalesce. One observation consumes the pending notification, and a
later trigger remains observable. Wake and resource events share the fixed event
capacity without silent loss.

## Mutation outcomes

A failed mutation reports what happened and returns or preserves a handle when
backend state may remain:

| Status | Register | Modify | Delete |
| --- | --- | --- | --- |
| `NotApplied` | Release the reservation; no handle | Preserve prior state | Return the handle in prior state |
| `Applied` | Return registered and armed | Commit and rearm | Return stale after retirement |
| `Unknown` | Return uncertain | Mark uncertain | Return uncertain and retryable |

An uncertain outcome is never presented as a successful rollback.
Capability-bearing errors return copyable registration handles by value.
