# ShieldSpec release-readiness audit — October 28, 2026

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

Before tagging:

1. Run the full workspace test, formatting and strict Clippy gates from the intended release commit; rerun pinned historical discovery if claiming newly verified upstream reproduction for that release.
2. Review public Rust APIs, snapshot/runner/report schema compatibility, MSRV 1.95, and the pinned prerelease Zcash SDK dependency matrix.
3. Decide whether release distribution means source-only CLI/Action or separately installable runner binaries. The current composite Action must remain accurately labeled snapshot-only unless its behavior is deliberately changed and tested.
4. Resolve crate publishability and internal dependency version alignment if crates.io distribution is intended; do not publish the `publish = false` runner/fuzzer as though they were packages.
5. Update changelog, release notes, examples and consumer pin to the actual immutable tag/commit, then check clean-checkout installation and CI on that exact ref.

The next meaningful product proof is external wallet-developer reproduction and a genuine different-revision comparison, not another controlled injected failure or a claim of a new upstream vulnerability.
