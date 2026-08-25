# zio-testkit

`zio-testkit` provides consumer-facing conformance scenarios for zio's
registration mutation and wake contracts. Its deterministic mutation suite
exercises every successful, not-applied, applied, and unknown branch of
register, modify, and delete without requiring operating-system fault
injection. Its black-box wake suite uses only zio's ordinary public poller API
against the host's native backend.

The crate is test infrastructure, is not published, and depends on the exact
workspace version of `zio` with its explicit `test-support` feature.

```rust
let report = zio_testkit::run_all();
report.into_result()?;
let wake_report = zio_testkit::run_wake_conformance();
wake_report.into_result()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
