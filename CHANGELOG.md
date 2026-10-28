# Changelog

## v0.2.0 — 2026-10-28

Source-only GitHub release of the standalone ShieldSpec developer tool. No crates.io packages or prebuilt binaries are distributed with this tag.

### Added since v0.1.0

- Generic two-process differential runner with observed wallet-policy verdict comparisons and coverage counts.
- Deterministic scenario generation, semantic shrinking, saved-case replay and executable SHA-256/argument provenance checks.
- Real Zcash SDK proposal runner, including activation-aware Ironwood scenarios, shielded-recipient and change behavior, transparent shielding, ZIP-317 fees and input-selection cases.
- Pinned historical Zallet `FullPrivacy` comparison with separately compiled before/after revisions and independently reproducible discovery/replay evidence. This reproduces a previously fixed upstream issue; it is not a newly discovered vulnerability.
- Developer onboarding, privacy model, CI guidance, reproducible evidence and read-only public explorer.

### Compatibility and limitations

- Rust 1.95.0 is pinned. The Zcash SDK and other upstream dependencies include exact prerelease versions; their APIs are not promised compatible with other SDK versions.
- Snapshot schema remains `schema_version: 1`; the runner accepts the original envelope and an optional `protocol_version: 1` envelope. The core checks and differential protocol are pre-1.0 interfaces, not a stability guarantee.
- The composite GitHub Action only compares normalized JSON snapshots. It does not build or fuzz wallets or upload code-scanning alerts.
- `shieldspec-fuzz` and `shieldspec-zcash-runner` are source-build-only (`publish = false`). No mainnet signing, broadcasting, anonymity guarantee or new upstream vulnerability claim is made.
- Cargo package-list checks succeeded for four publishable manifests; the core crate's `cargo publish --dry-run` succeeded at v0.1.0. Other dry-runs cannot resolve the unpublished `shieldspec-core` package from crates.io, so **no crates are published**.

See [release readiness and reproduction](docs/release-readiness.md) and [getting started](docs/getting-started.md).
