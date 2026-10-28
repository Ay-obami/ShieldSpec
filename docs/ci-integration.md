# Developer integration and CI

Start with the [quickstart](getting-started.md). These examples target `main` after PRs #42 and #43, not necessarily the older `v0.1.0` release. Pin the reviewed commit you actually test when adopting the Action. The checked-in composite `action.yml` is the same as at `v0.1.0`: it runs **only** snapshot `check`/`diff`, even when referenced from current `main`. See [release readiness](release-readiness.md) before treating newer source features as released.

## Rust integration: inspect before signing

In a wallet that already constructs an SDK `Proposal`, call `shieldspec_zcash::snapshot_from_proposal(&proposal, "testnet")`, then `shieldspec_core::assess(&snapshot, &shieldspec_core::Policy::standard())`. Handle adapter errors and review `assessment.passes` and `assessment.findings` before continuing your own signing flow. Supply the real network; the SDK proposal does not contain it. ShieldSpec does not intercept or authorize signing for you.

Use compatible versions of `shieldspec-core` and `shieldspec-zcash` from the same checkout. The executable [adapter tests](../crates/shieldspec-zcash/tests/proposal_adapter.rs) show real SDK proposal construction and assertions:

```bash
cargo test --locked -p shieldspec-zcash --test proposal_adapter
cargo test --locked -p shieldspec-cli --test golden
```

The current runner pins `zcash_client_backend =0.25.0-pre.2`, `zcash_client_sqlite =0.23.0-pre.2`, `zcash_keys =0.17.0-pre.2`, `zcash_protocol =0.11.0-pre.1`, `zcash_primitives =0.31.0-pre.2`, and `zcash_transparent =0.11.0-pre.2`. See [its manifest](../crates/shieldspec-zcash-runner/Cargo.toml) and the committed [Cargo.lock](../Cargo.lock). These prerelease APIs are not a promise of compatibility with arbitrary wallet SDK versions.

## A passing snapshot Action gate

This is a complete workflow for developing ShieldSpec itself; `uses: ./` runs the Action in the checked-out revision. A consumer repository should instead reference `Ay-obami/ShieldSpec@<reviewed-full-commit-sha>` and supply its own normalized snapshots. The placeholder is not a literal runnable ref.

```yaml
name: Proposal privacy
on: [push, pull_request]
permissions:
  contents: read
jobs:
  privacy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683
      - name: Check proposal
        uses: ./
        with:
          candidate: fixtures/shielded-payment.json
          policy: standard
          format: human
```

Set `baseline` to compare rather than assess one snapshot. Paths are relative to the caller's `GITHUB_WORKSPACE` (absolute paths also work). `policy` accepts `standard` or `strict`; `format` accepts `human`, `json`, or `sarif`. The Action installs Rust 1.95.0 and compiles the snapshot CLI with `--locked`; it does not build a wallet, generate proposals, or run fuzzing. Its `status` output is `0` for pass, `1` for an operational error, or `2` for a configured privacy failure. Nonzero status fails the step.

For an intentionally failing regression and an assertion on `steps.regression.outputs.status`, copy the repository's [Action demo workflow](../.github/workflows/shieldspec-demo.yml). Do not use `continue-on-error` on your production gate unless another step explicitly enforces the result.

## SARIF is output, not an automatic upload

`format: sarif` writes SARIF to the job log. The composite Action does not create a report file or upload code-scanning results. To retain a report, invoke the CLI in a checkout containing ShieldSpec source and preserve its exit status:

```bash
set +e
SHIELDSPEC_SOURCE_ROOT="$PWD" cargo run --locked -p shieldspec-cli -- \
  diff --baseline fixtures/shielded-payment.json \
  --candidate fixtures/transparent-change.json \
  --policy standard --format sarif > shieldspec.sarif
status=$?
set -e
# Upload shieldspec.sarif in a subsequent always-run artifact/SARIF step.
exit "$status"
```

This known candidate exits `2`. In a multi-step CI job, use `if: always()` for report preservation, and keep the original gate's failure. An optional code-scanning upload requires a separately configured uploader, the relevant repository support, and its permissions; generating SARIF alone does not publish alerts.

## Differential campaigns are a separate gate

Build `cargo build --locked -p shieldspec-zcash-runner --bins`, then follow the [campaign and replay commands](getting-started.md#2-run-a-real-sdk-differential-campaign). Use independently compiled, protocol-compatible baseline and candidate runners for an actual revision comparison; identical binaries are only a control. Keep their bytes, exact arguments, build revisions and lockfiles with the result. `--baseline-sdk` and `--candidate-sdk` are declared labels, not source verification.

For opt-in transparent-UTXO scenarios, add `--transparent-funding` to both seed-based campaign and seed/ordinal replay commands. Do not combine it with `--scenario-family ironwood`. A saved `--input` discovery carries its scenario and provenance; seed-only replay must preserve the generator family explicitly. `--save` writes only a stable minimized discovery, not a file for every clean run.

A fuzzer exit `2` can represent either a snapshot regression or a changed actual named wallet-policy decision; rejection may be a desired fix. Exit `1` is an operational failure, not a valid wallet rejection. Never silently turn either into a passing privacy verdict. Read the [runner contract](runner-protocol-contract.md) and [wallet-policy observations](wallet-policy-observations.md).

The original [historical Zallet witness](../evidence/historical-zallet-677/README.md) remains pinned and separate. The new [Zallet automatic-discovery workflow](../.github/workflows/zallet-auto-discovery.yml) compiles the two real `FullPrivacy` implementations, runs controls, verifies the general fuzzer's previously documented historical difference and its SHA-linked minimized replay, and uploads artifacts. [Reproduce locally](zallet-auto-discovery.md). The [librustzcash #2614 witness](../evidence/historical-2614/README.md) establishes SDK correctness history, not a proven privacy leak. A clean campaign is evidence only for exercised cases and configured rules, not an anonymity guarantee.
