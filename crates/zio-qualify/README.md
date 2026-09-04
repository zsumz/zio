# zio-qualify

`zio-qualify` is the workspace-private readiness qualification harness for
`zio`'s owned and borrowed tiers, Mio, and `polling`. Each candidate receives a
fresh native fixture and is checked independently against the same declared
contract. No candidate is an oracle for another.

The matrix records exact delivery semantics and proves quiet, activation,
delivery, cardinality, one-shot disarm, rearm, operation, and cleanup behavior.
Structured failures retain every failed phase.

Run bounded timing and allocation smokes separately:

```sh
cargo run -p zio-qualify --bin zio-perf -- --smoke
cargo run -p zio-qualify --no-default-features --features allocation-metrics \
  --bin zio-perf-alloc -- --smoke
```

Timing builds do not link the allocation instrumentor. Receipts contain one
metric kind only.

On a kqueue host, run the review-defined registration/event-capacity skew gate:

```sh
cargo run -p zio-qualify --release --no-default-features \
  --features kqueue-skew --bin zio-kqueue-skew -- \
  --output target/kqueue-skew.ndjson
```

See [Performance](../../docs/performance.md) for the receipt format and method.
This crate is unpublished and absent from `zio`'s production graph.
