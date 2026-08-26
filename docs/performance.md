# Performance

`zio-perf` compares Zio, Mio, and `polling` without treating one as the oracle.
It is workspace-private and never enters Zio's production graph.

## Run

```sh
cargo run -p zio-qualify --release --no-default-features \
  --bin zio-perf -- --output target/timing.ndjson
cargo run -p zio-qualify --release --no-default-features \
  --features allocation-metrics \
  --bin zio-perf-alloc -- --output target/allocation.ndjson
```

Use `--help` for stable scenario names and bounded filters. `--smoke` validates
the runner; it is not a useful timing sample.

## Method

- a pilot targets 100 ms per candidate, then every candidate uses the largest
  required iteration count;
- three shared-iteration warmup batches precede 90 balanced timing rounds;
- allocation uses 12 rounds in a separately instrumented binary;
- allocation counters are thread-local. Blocked-wake allocation receipts cover
  the waiter; pretriggered wake covers trigger and observation together;
- raw receipts retain round, candidate position, operations, events, and metric
  values;
- summaries report sample-mean throughput distributions. Their p95 is not an
  operation-latency percentile;
- allocation and timing are normalized by operation and event;
- live-descriptor stages and whole-sample retained deltas expose setup cost and
  leaks;
- every receipt records versions, source state, toolchain, host, capacities,
  delivery, calibration, and measured scope.

Construction is split into poller-only and poller-with-waker at capacities 1,
64, and 1024. Registration measures combined lifecycle plus isolated 64-item
register and delete segments. Readiness measures initial lifecycle and
persistent-registration cycles separately at 1, 64, and 1024 events. Wake
measures pretriggered and blocked cross-thread delivery.

Zio initial readiness uses level delivery, Mio uses its native default, and
`polling` uses its native one-shot default. Persistent `polling` runs only when
the host reports native level support. The one-shot rearm absence probe is
outside the measured segment.

Zio's configured storage and retained descriptor duplicate are included in its
measurements. Large batches are skipped with a structured reason when the
process file-descriptor limit is too small.

## Evidence

The `Performance qualification` workflow records five independent Linux and
macOS replicas. Linux also records lifecycle, persistent-readiness, and wake
syscall summaries; traced timing output is discarded. Timings are evidence, not
a CI threshold. Compare equivalent hosts and inspect raw paired rounds before
drawing conclusions.
