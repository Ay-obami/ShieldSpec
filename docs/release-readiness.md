# ShieldSpec release-readiness audit — October 28, 2026

> **Release follow-up (October 28):** A separate `v0.2.0` source-only release candidate aligns all six Cargo package versions and matching internal dependency versions. The earlier audit below records the pre-release `v0.1.0` state at its original commit and remains historical. On an isolated laptop checkout with Rust 1.95.0, `cargo package --list` enumerated core (15 files), testkit (18), CLI (16) and Zcash (8). `cargo publish --dry-run --locked -p shieldspec-core` passed for the original v0.1.0 manifest. Dry-runs of testkit, CLI and Zcash failed because unpublished `shieldspec-core` cannot be resolved in crates.io. This is **not** authorization for crate publication; the v0.2.0 distribution is GitHub source only. See [changelog](../CHANGELOG.md).

This is an evidence-bound audit of the standalone developer tool, not a release announcement. The audited fresh clone was at `5e2f82eb09d6d4ae089b6ec156528937d144301d` on `main`. Record a new full commit SHA for any later verification; `main` is mutable.

## Verification and reproduction

| Gate | Evidence | Outcome and boundary |
| --- | --- | --- |
| Fresh-checkout Python harness | `python3 -m unittest discover -s scripts/tests -v` | 26 tests passed locally on this audit checkout, including Zallet runner adapters, fail-closed replay validation, and historical test injection. These are not a substitute for building upstream Zallet. |
| Fresh-checkout static site | `node --test site/tests/site.test.mjs` | Site suite passed locally (1 Node test file). |
| Rust format, workspace tests, strict Clippy | [CI run 40746440060](https://github.com/Ay-obami/ShieldSpec/actions/runs/40746440060) at `5e2f82e` | Passed on GitHub-hosted fresh checkout, including real Zcash proposal runner and deterministic differential smoke. Rust was not installed in this local audit environment, so a separate local Rust run is **not claimed**. |
| Independently compiled pinned Zallet historical comparison | [Historical Zallet workflow 40616469538](https://github.com/Ay-obami/ShieldSpec/actions/runs/40616469538) at `8d9015e` | Passed two source-revision compilations, actual `FullPrivacy` controls, general-fuzzer discovery, semantic minimization and immutable SHA-linked replay. This is prior CI evidence at a named commit, **not a new October 28 recompilation**. See [full reproduction recipe](zallet-auto-discovery.md). |

To independently reproduce the historical result from a clean machine with Rust 1.95, Python 3, network access and the native upstream dependencies, follow the exact [pinned-runner build, discovery and replay commands](zallet-auto-discovery.md). Preserve the runner SHA-256 identities and raw CI artifacts; do not replace a real policy observation with a fixture-only verdict. The Zallet difference was documented and fixed upstream, not newly discovered by ShieldSpec.

## What `v0.1.0` ships

The published [v0.1.0 release](https://github.com/Ay-obami/ShieldSpec/releases/tag/v0.1.0) is an older source tag with no attached binary assets. The composite `action.yml` has **no changes** between that tag and the audited `main` checkout. It installs pinned Rust and runs `shieldspec-cli check` or `diff` on caller-supplied normalized JSON. It supports human, JSON and SARIF output to logs, with exit codes 0/1/2. It does **not** build a wallet, run the differential fuzzer, upload SARIF or guarantee anonymity.

The generic runner protocol, observed policy comparisons, expanded real SDK corpus, Zallet auto-discovery and saved-case replay have all been developed **after** `v0.1.0`. The crates still declare `0.1.0` in their manifests; these manifest values are not proof that the post-tag features were published as packages. Both `shieldspec-fuzz` and `shieldspec-zcash-runner` explicitly set `publish = false`.

## Release recommendation

**Prepare a separately reviewed `v0.2.0` candidate; do not move the existing `v0.1.0` tag.** The new differential and policy-observation surface is a substantial pre-1.0 feature expansion. This is a recommendation only: no version bump, package publication, tag or release was created by this audit.

### Compatibility inventory for a source release

| Consumer interface | Current contract | Release risk / required check |
| --- | --- | --- |
| Snapshot JSON (`shieldspec-core`) | `schema_version: 1`; `InputFact.origin` defaults to `wallet` when omitted; snapshots reject unknown fields | Compare `v0.1.0` fixture inputs and expected findings against the release candidate; document any changed rule behavior. |
| Snapshot CLI and composite Action | `check`/`diff`, policies `standard`/`strict`, formats `human`/`json`/`sarif`; exit `0`/`1`/`2` | Retain golden-output and downstream Action smoke gates. The Action has no fuzzer or code-scanning upload. |
| Differential JSON runner boundary (`shieldspec-fuzz`) | Legacy unversioned envelope or optional explicit `protocol_version: 1`; `ok`, `observed`, `error` | Keep legacy decoding and first-class actual policy observations distinct; test both sides of the versioned protocol. |
| Saved discovery/replay | Scenario family, original/minimized inputs, runner SHA-256 and exact arguments | Verify replay fails closed on changed binaries, policy names, argument identities, or verdict direction. |
| Real SDK adapter (`shieldspec-zcash`) | `zcash_client_backend =0.25.0-pre.2` and `zcash_protocol =0.11.0-pre.1` | Exact prerelease SDK pins are not broad semver compatibility; rebuild consumer examples and test against any future SDK revision deliberately. |
| Real runner (`shieldspec-zcash-runner`) | Further exact prerelease pins for SQLite, keys, primitives, transparent and ZIP-321; synthetic wallet state | Validate Rust 1.95 and native build requirements on clean Linux; independent wallet builds need compatible JSON, not matching Rust types. |

### Distribution decision

For the next release, **source-only GitHub release plus the existing snapshot-only composite Action** is the lowest-risk candidate. The repository can document a source-built `shieldspec-fuzz` command without claiming a published binary or crates.io distribution. If downloadable runner binaries or published crates are desired later, that is a separate packaging and supply-chain task.

| Crate | Manifest version at audit | crates.io release posture |
| --- | --- | --- |
| `shieldspec-core` | `0.1.0` | Default publishable; no package dry-run or registry availability verification performed |
| `shieldspec-testkit` | `0.1.0` | Default publishable; depends on `shieldspec-core` at matching version |
| `shieldspec-cli` | `0.1.0` | Default publishable; depends on matching internal core/testkit versions |
| `shieldspec-zcash` | `0.1.0` | Default publishable; depends on matching core and exact prerelease Zcash SDK |
| `shieldspec-fuzz` | `0.1.0` | Explicitly `publish = false`; source-build only |
| `shieldspec-zcash-runner` | `0.1.0` | Explicitly `publish = false`; source-build only |

There is no evidence here that the four default-publishable packages already exist in the registry. Publishing them would require package dry-runs and an ordered release of internal dependencies, not just changing manifest versions. For a **source-only** `v0.2.0`, the version in crate manifests should be aligned with the chosen release policy before tagging, but publishing to crates.io is optional.

Before tagging:

1. Run the full workspace test, formatting and strict Clippy gates from the intended release commit; rerun pinned historical discovery if claiming newly verified upstream reproduction for that release.
2. Review public Rust APIs, snapshot/runner/report schema compatibility, MSRV 1.95, and the pinned prerelease Zcash SDK dependency matrix.
3. Decide whether release distribution means source-only CLI/Action or separately installable runner binaries. The current composite Action must remain accurately labeled snapshot-only unless its behavior is deliberately changed and tested.
4. Resolve crate publishability and internal dependency version alignment if crates.io distribution is intended; do not publish the `publish = false` runner/fuzzer as though they were packages.
5. Update changelog, release notes, examples and consumer pin to the actual immutable tag/commit, then check clean-checkout installation and CI on that exact ref.

Suggested candidate verification commands, run on the **exact intended tag commit**:

```bash
cargo fmt --check
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --manifest-path examples/mock-wallet/Cargo.toml --all-targets
python3 -m unittest discover -s scripts/tests -v
node --test site/tests/site.test.mjs
```

For an eventual crates.io decision, run `cargo package --list` and `cargo publish --dry-run` in dependency order on each publishable crate, resolving unpublished internal dependencies first. These commands were **not** run in this audit environment; do not infer successful packaging from green tests. The separately pinned [historical Zallet workflow](../.github/workflows/zallet-auto-discovery.yml) remains opt-in when its upstream compilation dependencies are available.

The next meaningful product proof is external wallet-developer reproduction and a genuine different-revision comparison, not another controlled injected failure or a claim of a new upstream vulnerability.
