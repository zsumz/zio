# Contracts

This page defines zio's portable behavior. Native readiness is advisory; the
matching nonblocking operation is always authoritative.

## API evolution

`Error`, `Operation`, `CapacityKind`, and `CapacityReason` are open diagnostic vocabularies.
Downstream matches must include a fallback arm. `Event`, `CommitStatus`,
`DescriptorOwnership`, `Mode`, `Wait`, `ArmState`, `RegistrationState`,
`RegisterOwnedError`, and `DeleteOwnedError` are closed domains; case changes are
breaking. Event fields may grow; match with `..`.

`Operation` names only failures a current backend can report.
`Error::UnsupportedPlatform` has no associated operation.

Use `Error` accessors for structured details. Match `Error::Capacity` with `..`;
its diagnostic fields may grow.

Public value layouts and flag encodings are opaque.

`BackendLimit` rejects capacities that native or token representations cannot hold.
`Error::capacity_limit` reports the configured or attempted logical capacity.

## Registration ownership

`Poll::register` is the safe default. Each successful call retains a distinct
descriptor duplicate and exact generation, even for repeated registration of
one source or its duplicated handles. Caller close and numeric descriptor reuse
cannot redirect later mutations. Handles are poller-scoped; another poller
rejects them. Keys need not be unique. Each resource event carries its exact
registration handle.

`Poll::register_owned` transfers an `OwnedFd` without duplication. Rejected and
`NotApplied` calls return it; `Applied` and `Unknown` failures return the
retained registration.
`Poll::delete_owned` retires an owned registration and returns its exact
descriptor. Borrowed registrations are rejected before backend work. An
`Applied` failure returns the descriptor; other failures return the attempted
handle. Inspect the cause before reuse.
Dropping a poller closes retained owned descriptors; borrowed descriptors
remain caller-owned.
`Poll::registration_fd` safely borrows any retained resource descriptor,
including one in uncertain backend state.
`Poll::registrations` returns a bounded snapshot. `Poll::iter_registrations`
borrows the same set without allocating. Both have unspecified order.
`RegistrationInfo::descriptor_ownership` reports who owns the retained descriptor.
Full means no slot is reservable. Remaining capacity excludes live and
generation-exhausted slots. `CapacityReason::GenerationExhausted` means only a
new poller can restore registration capacity.

`Poll::set_key` changes only future resource-event routing and does no backend work.
`Poll::modify_with_key` settles key, interest, and mode under one commit outcome.
`RegistrationState::arm` returns `None` when backend state is uncertain.
On supported targets, `Poll` implements `AsFd` and `AsRawFd`; selector
readability means a nonblocking wait may observe an event.

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
Registration IDs support comparison, hashing, and diagnostic display.

Successful deletion retires the generation and makes every copy stale. An
`Applied` delete failure does the same; `NotApplied` preserves every copy's
prior state; `Unknown` makes every copy uncertain and allows a delete retry.
Stale and wrong-poller copies are rejected before backend work and cannot affect
a reused slot.

`Poll::delete_all` validates retained handles, then stops at the first failure.
Earlier deletions may have succeeded; later entries are untouched.

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
is reused. It borrows as the ordered outcome slice and iterates by reference.
`WaitReport::is_complete` means no reconciliation is needed.
After processing events, `WaitReport::into_result` supports direct propagation.
Returning `Err` means delivery failed and leaves `Events` empty.
`Events::is_full` describes the delivered batch; it does not prove more
readiness is pending.

## Allocation contract

Unsupported targets reject poll construction before capacity validation or allocation.
Poll construction retains native-event, coalescing, mutation, receipt, and
ownership scratch. Successful waits reuse it without growing zio-owned heap
storage. Wake, observation, registration iteration, and successful bulk
deletion allocate nothing.

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
collapse to nonblocking; `Wait::is_nonblocking` recognizes both forms. A backend
may round up to its supported resolution, and scheduling may delay return. Linux
rounds up to milliseconds. Kqueue uses nanosecond fields. Large limits are
clamped to the backend integer range. `Poll::wait_until` computes a monotonic
remaining duration at entry; a reached deadline is nonblocking. An interrupted
wait returns `Error::Io`. `Error::is_wait_interrupted` classifies only that
non-mutation case.

Use nonblocking descriptors and perform I/O until it would block.

## Wake behavior

`Poll` is `Send` but not `Sync`. `Waker` is `Send + Sync`.

A poller binds its wake source to one key. Same-key requests and clones share
it, and each `Waker` reports that key. A conflicting key is rejected without
replacing the original. `Poll::waker_key` reports the current binding.
`Waker::will_wake` compares keyed poller destinations.

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
Capability-bearing errors return copyable handles and implement `AsRef<Error>`.
