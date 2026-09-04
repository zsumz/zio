# Changelog

All notable changes are recorded here. zio follows Semantic Versioning once a
stable release exists.

## Unreleased

### Changed

- Fill kqueue event batches with cyclic fair selection while preserving
  first-observation output order.
- Allocate virgin registration slots before recycling retired slots, and
  recycle them in FIFO order to distribute generation churn.
- Validate safe registration capacity before duplicating a descriptor.
- Make `Poll::rearm` use one validated mutation path and benchmark that public
  operation directly.
- Make `Mode` non-exhaustive so future delivery modes do not require a major
  release.
- Keep custom construction on the named `Poll::builder` API.

### Fixed

- Make the packaged external-consumer rehearsal inspect non-exhaustive events
  through public predicates instead of constructing a variant.
- Describe wake observation as consuming the logical pending notification.

### Policy

- Define the Rust 1.88 MSRV policy and semver exemption for
  `unstable-test-support`.
