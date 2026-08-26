# Performance

`zio-perf` compares Zio, Mio, and `polling` without treating one as the oracle.
It is workspace-private and never enters Zio's production graph.

## Run

```sh
cargo run -p zio-qualify --release --bin zio-perf -- --samples 12
cargo run -p zio-qualify --release --no-default-features \
  --features allocation-metrics \
  --bin zio-perf-alloc -- --samples 12
```

Use `--help` for stable scenario names and bounded filters. `--smoke` validates
the runner; it is not a useful timing sample.

## Method

- each candidate gets the same scenario and independently created fixtures;
- 12 rounds balance each candidate across measurement positions;
- warmup is excluded;
- uninstrumented timing and instrumented allocation run in separate binaries;
- timing receipts retain raw samples, median, p95, and median absolute deviation;
- allocation receipts retain counts and bytes; both retain operations, events,
  and file-descriptor deltas when the host exposes them;
- candidate versions, commit and dirty state, toolchain, OS, architecture, CPU,
  capacities, and delivery semantics are recorded in every NDJSON receipt.

Initial readiness compares one observation only. Zio uses explicit level
delivery, Mio uses its native default, and `polling` uses its native one-shot
default. Native level and one-shot cases run only where the candidate exposes
those modes.

Zio's configured storage and retained descriptor duplicate are included in its
measurements. Large batches are skipped with a structured reason when the
process file-descriptor limit is too small.

## Evidence

The `Performance qualification` workflow records Linux and macOS timing and
allocation receipts. Linux also records syscall summaries; traced timing output
is discarded. Timings are evidence, not a CI threshold: compare like-for-like
receipts from equivalent hosts and inspect dispersion before drawing conclusions.
