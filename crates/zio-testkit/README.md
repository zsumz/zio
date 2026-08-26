# zio-testkit

Reference conformance for zio consumers.

- Mutation scenarios cover every register, modify, and delete outcome without
  operating-system fault injection.
- Replayable model sequences check bounded mutation histories after every step.
- Wake and readiness scenarios exercise zio's public API on the native backend.
- Reports use stable scenario names and structured failures.

The readiness matrix covers Unix streams, TCP streams, pipes, readable,
writable, combined, level, and one-shot behavior. Repository tests add
refused-connect and abortive-reset fixtures.

This crate is workspace-private, unpublished, and absent from normal zio
builds. Its public API exposes no raw descriptors, syscall types, or native
backend trait.

```rust
zio_testkit::run_all().into_result()?;
zio_testkit::run_wake_conformance().into_result()?;
zio_testkit::run_readiness_conformance().into_result()?;
zio_testkit::run_model_sequences().into_result()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```
