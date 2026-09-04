# Performance

`zio-perf` compares zio's owned and borrowed tiers, Mio, and `polling` without
treating one as the oracle. It is workspace-private and never enters zio's
production graph.

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
- three shared-iteration warmup batches precede 96 balanced timing rounds;
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

Both zio tiers use level delivery for initial readiness, Mio uses its native
default, and `polling` uses its native one-shot default. Persistent `polling`
runs only when the host reports native level support. The one-shot rearm
absence probe is outside the measured segment.

zio's configured storage is included. Its safe tier also includes the retained
descriptor duplicate; its unsafe tier borrows each caller-owned descriptor.
Initial readiness includes registration and deletion. Persistent readiness
isolates the already-registered hot path. Large batches are skipped with a
structured reason when the process file-descriptor limit is too small.

## Kqueue skew gate

Kqueue currently requests enough native space to observe both filters for every
registration, then delivers at most the configured logical event capacity. That
preserves complete split-filter snapshots and zio-controlled fairness, but its
cost scales with the ready registration set rather than only the delivered
batch.

Before stable 1.0, run the following matrix on macOS and at least one BSD. These
are dedicated-host measurements, not ordinary hosted-runner thresholds.

| Registrations | Event capacity | Ready fraction |
| ---: | ---: | ---: |
| 100,000 | 64 | 0.1% |
| 100,000 | 256 | 1% |
| 100,000 | 256 | 50% |
| 100,000 | 1,024 | 100% |
| 1,000,000 | 1,024 | sparse |

The `sparse` row makes 1,024 of the million registrations ready. Run the fixed
matrix with:

```sh
run_id="$(uuidgen | tr '[:upper:]' '[:lower:]')"
cargo run -p zio-qualify --release --no-default-features \
  --features kqueue-skew --bin zio-kqueue-skew -- \
  --run-id "$run_id" \
  --output target/kqueue-skew.ndjson
```

Each row measures one complete fair cycle for level and one-shot delivery. Its
`zio.kqueue-skew.v1` receipt retains raw native events returned, logical events
delivered, nanoseconds per delivered event, waits required to complete the
cycle, receipt-checked one-shot disarm submission cost, and currently retained
heap bytes. It also records the file-descriptor limit and skips a row explicitly
when the host cannot support it. The explicit run UUID binds all five rows to
one dedicated-host capture. `--smoke` replaces the matrix with five
registrations, capacity two, and three ready registrations.

The runner uses the semver-exempt `unstable-test-support` wait counters; they are
not stable application API. The results decide whether 1.x preserves complete
snapshots or adopts batch-bounded native collection with a correspondingly
weaker coalescing contract.

## Evidence

The `Performance qualification` workflow records five independent Linux and
macOS replicas. Linux also records lifecycle, persistent-readiness, and wake
syscall summaries; traced timing output is discarded. Timings are evidence, not
a CI threshold. The checked-in `crates/zio-qualify/perf-catalog.json` is
byte-for-byte synchronized with the Rust candidate and scenario model.

Each successful matrix job emits a `zio.performance-qualification.v2` summary.
The recorder requires the exact 78-pair catalog in both raw files, 96 timing
samples, 12 allocation samples, release binaries, every zio tier passing, and
only catalogued peer limitations reported as unsupported. The summary retains
SHA-256 digests of both raw files and the catalog, their recording timestamps,
host and toolchain identity, and the GitHub run, attempt, job, OS, and replica.
Stable qualification reopens the sibling raw files and reproduces every
summary before accepting it. Compare equivalent hosts and inspect raw paired
rounds before drawing conclusions.
