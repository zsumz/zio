# zio-testkit

`zio-testkit` provides deterministic, consumer-facing conformance scenarios for
zio's registration mutation contracts. It exercises every successful,
not-applied, applied, and unknown branch of register, modify, and delete without
requiring operating-system fault injection.

The crate is test infrastructure, is not published, and depends on the exact
workspace version of `zio` with its explicit `test-support` feature.

```rust
let report = zio_testkit::run_all();
report.into_result()?;
# Ok::<(), zio_testkit::MutationReport>(())
```
