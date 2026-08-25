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
| Linux | Native tests through the canonical graph |
| macOS | Native kqueue tests, Clippy, and rustdoc |
| FreeBSD, NetBSD | MSRV checks and pinned-toolchain Clippy cross-builds |

BSD support remains experimental until native execution is hosted.

## Dependency roles

Normal zio builds depend only on target-gated `libc`.

Development uses:

- `mio` as an independent readiness oracle;
- `socket2` to create safe refused-connect and abortive-reset fixtures;
- `allocation-counter` to measure wait-path allocation;
- `zio-testkit` for reusable mutation, wake, and readiness conformance.

None enters zio's production dependency graph.
