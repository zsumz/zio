# Contributing

Keep changes focused on zio's bounded Unix readiness contract. New platforms,
delivery modes, or runtime facilities need a concrete consumer requirement and
a qualification plan before implementation.

## Local checks

Use the pinned tools and run the same graph as CI:

```sh
zcheck run check --jobs 2
zrail diff --base origin/main --deny-grants
```

The stable-evidence task and its Rust integration test require Bash, `jq`, and
`shasum` on the developer host.

See [Qualification](docs/qualification.md) for platform and evidence lanes.
Changes to ownership, registration generations, unsafe code, event storage, or
kqueue receipts should include focused failure-path tests and an updated safety
argument beside the affected code.

Do not weaken a gate to make a change pass. Keep version changes, tags, and
publication out of ordinary feature work.

## Compatibility

Rust 1.88 is the MSRV. Public API evolution and the semver-exempt
`unstable-test-support` feature are defined in
[Contracts](docs/contracts.md#api-evolution). Add user-visible changes to the
Unreleased section of [CHANGELOG.md](CHANGELOG.md).
