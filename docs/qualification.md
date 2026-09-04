# Qualification

Run the same graph as CI:

```sh
zcheck run check
```

Review authority changes separately:

```sh
zrail diff --base HEAD --deny-grants
```

Use zcheck 0.0.2 and zrail 0.0.3-rc.4. These exact versions match CI and the
reviewed zrail lock.

## Coverage

The canonical graph checks architecture, repository policy, formatting,
Clippy, rustdoc, doctests, Rust 1.88 and pinned-toolchain tests, package
contents, and diff hygiene.

| Platform | Evidence |
| --- | --- |
| Linux | Native tests plus 32-bit x86 and 64-bit Arm API checks |
| macOS | Native kqueue tests plus 64-bit Arm and x86 API checks |
| FreeBSD, NetBSD | Pinned native guest workflow plus MSRV and Clippy cross-builds |

Each native BSD evidence bundle retains the guest release, toolchains, source
commit, verified `rustup-init` checksum, logs, and machine-readable result.

## Model

`zio-testkit` replays a fixed 64-seed, 64-action mutation corpus. It checks every
step against an independent state model, retains the first failing prefix, and
pins focused seeds for outcome, rearm, stale-generation, and wrong-poller
behavior. Invalid-interest actions must do no backend work or consume a
generation.

Wake and kqueue recovery stay separate evidence lanes because they exercise
native delivery and post-observation recovery rather than the mutation reducer.

## Peer matrix

`zio-qualify` runs zio's owned and borrowed tiers, Mio, and `polling`
independently against the same readiness contracts. Receipts state each
candidate's ownership and delivery semantics; agreement is never the oracle.

See [Performance](performance.md) for the reproducible benchmark method.

## Release rehearsal

```sh
zcheck run release
```

The release graph starts with the canonical gate, then verifies the clean crate
archive, VCS provenance, package contents, MSRV/current extracted tests,
rustdoc, and an independent consumer. It does not publish or tag.

Stable 1.0 additionally requires the native kqueue skew matrix in
[Performance](performance.md), performance qualification, and release rehearsal
to run against the exact candidate commit. Hosted checks, native guest receipts,
and packaged-artifact evidence are separate gates.

The kqueue skew runner emits exact source and host provenance and structured
unsupported rows when the descriptor limit is insufficient. A complete gate
requires passed rows on macOS and at least one BSD; unsupported receipts are
diagnostic evidence, not a substitute for that native coverage.

## Dependency roles

Normal zio builds use target-gated `libc` and Rustix's Linux epoll syscall
layer.

Development uses:

- `mio` and `polling` as independent readiness comparators;
- `socket2` to create safe refused-connect and abortive-reset fixtures;
- `allocation-counter` to measure test and benchmark allocations;
- `zio-testkit` for mutation, model, wake, and readiness conformance;
- `zio-qualify` for the private peer matrix and benchmark runner.

None of these development dependencies enters zio's production graph.
