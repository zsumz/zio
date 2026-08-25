# zio-testkit

`zio-testkit` provides consumer-facing conformance scenarios for zio's
registration mutation, wake, and native readiness contracts. Its deterministic
mutation suite exercises every successful, not-applied, applied, and unknown
branch of register, modify, and delete without requiring operating-system
fault injection. Its black-box wake suite uses only zio's ordinary public
poller API against the host's native backend. Collectively, its black-box
readiness scenarios cover Unix streams, TCP streams, and anonymous pipes;
readable, writable, and combined interests; and both level and one-shot modes.

The reusable report needs no additional native-fixture dependencies and omits
two failure fixtures. Repository tests use safe `socket2` setup for an
in-progress refused connect and linger-zero abortive TCP reset qualification.

The crate is test infrastructure, is not published, and depends on the exact
workspace version of `zio` with its explicit `test-support` feature.

```rust
let report = zio_testkit::run_all();
report.into_result()?;
let wake_report = zio_testkit::run_wake_conformance();
wake_report.into_result()?;
let readiness_report = zio_testkit::run_readiness_conformance();
readiness_report.into_result()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
