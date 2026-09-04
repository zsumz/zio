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

## Stable 1.0 evidence gate

Stable release evidence is accepted only through the repository validator:

```sh
scripts/qualify-1.0 \
  --commit "$(git rev-parse HEAD)" \
  --version "1.0.0-rc.1" \
  --macos-receipts ../zio-evidence/kqueue/macos.ndjson \
  --bsd-receipts ../zio-evidence/kqueue/freebsd.ndjson \
  --performance-receipts ../zio-evidence/performance \
  --release-rehearsal-receipt ../zio-evidence/release/receipt.json
```

Use the exact intended version: `1.0.0-rc.N` while qualifying a release
candidate and `1.0.0` for stable. Before reading evidence, the validator
requires the repository checkout to be clean, requires `HEAD` to equal
`--commit`, and resolves the workspace's zio version through Cargo metadata to
prove that it equals `--version`.

The macOS and BSD inputs must each contain the five exact full-matrix rows,
with no missing, duplicate, unsupported, failed, dirty, malformed, or
foreign-commit receipt. The validator requires release builds, one consistent
host/run context, a distinct generated run UUID for each platform, integer
metrics, complete fair cycles, and exact one-shot disarm counts.

The performance path must contain all five Linux and five macOS artifact
directories. Every `qualification-receipt.json` must retain sibling
`timing.ndjson` and `allocation.ndjson` files. The gate revalidates their exact
catalogs, pass states, sample counts, release profile, host/toolchain context,
and SHA-256 digests, then rejects reused raw bytes across replica labels or
summaries from different workflow runs. The release rehearsal, performance
evidence, and both native kqueue runs must name the same candidate SHA and crate
version.

`scripts/test-qualify-1.0` constructs valid evidence and attacks both producer
and consumer. It covers invented or mismatched benchmark pairs, invalid pass
states, sample/profile drift, malformed numeric types, raw-file tampering,
reused replica bytes, mixed workflow runs, incomplete matrices, dirty or
foreign commits, and a mismatched release rehearsal.

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
