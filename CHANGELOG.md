# Changelog

## v0.3.0 — 2026-10-28 (GitHub Action/source release)

- Opt-in differential GitHub Action with a built-in first-party Zcash SDK negative-control preset and caller-built custom runner support.
- Deterministic campaign configuration, JSON reports, minimized discoveries and GitHub Actions evidence artifact upload before CI enforces status 0/1/2.
- Snapshot mode remains the default and backwards compatible.
- **Distribution boundary:** no v0.3.0 crates.io packages or prebuilt binaries; the four independently versioned published crates remain at 0.2.0. See [v0.3.0 release notes](docs/releases/v0.3.0.md).

## v0.2.0 — 2026-10-28

GitHub source release of the standalone ShieldSpec developer tool. Following the original tag, `shieldspec-core`, `shieldspec-testkit`, `shieldspec-cli`, and `shieldspec-zcash` version `0.2.0` were published to crates.io in dependency order. No prebuilt binaries are attached to the GitHub release.

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
- At the original source-tag audit, dependent crates could not resolve the then-unpublished core from crates.io. The subsequent ordered publication resolved this: all four version `0.2.0` packages passed package verification and were uploaded successfully. Both `publish = false` crates remain source-build-only.

See [release readiness and reproduction](docs/release-readiness.md) and [getting started](docs/getting-started.md).
