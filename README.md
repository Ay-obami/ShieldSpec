# ShieldSpec

**Privacy regression testing for Zcash wallet transaction proposals.** ShieldSpec helps wallet and SDK developers catch proposal-level privacy changes before signing or broadcast. It checks explicit privacy invariants, runs deterministic scenarios through real Zcash SDK proposal construction, and compares independently built baseline and candidate runners. Differences can be minimized and saved for verified replay.

ShieldSpec is developer tooling, not a wallet, anonymity score, signer, or transaction broadcaster. A passing check means only that the configured properties held for the inputs exercised.

- [Simple workflows](docs/simple-workflows.md): install CLI v0.4.0 and run `shieldspec try` or config-driven `shieldspec test` for snapshots and differential runners
- [On-site developer guide](https://ay-obami.github.io/ShieldSpec/docs.html): guided installation, copyable commands, expected results, real SDK comparisons, historical Zallet reproduction, and CI setup
- [Get started in the repository](docs/getting-started.md): detailed commands, expected outputs, exit codes, and CI integration
- [Privacy model](docs/privacy-model.md): what snapshots and actual wallet-policy verdicts do—and do not—establish
- [Wallet-policy observations](docs/wallet-policy-observations.md): runner protocol, real SDK Orchard-only mode, and NU6.3 campaign
- [Historical Zallet automatic rediscovery](docs/zallet-auto-discovery.md): build pinned independent FullPrivacy runners, run a real three-case differential campaign, and replay its minimized discovery
- [Roadmap](docs/roadmap.md): shipped features versus integration and future work
- [Release readiness](docs/release-readiness.md): verification evidence, tagged-Action scope, and next-version checklist
- [Live developer website](https://ay-obami.github.io/ShieldSpec/) · [site source and local preview](site/README.md)

The [developer website](https://ay-obami.github.io/ShieldSpec/) explains the privacy problem, checked properties, comparison workflow, reproducible result artifacts, and historical evidence. Its [step-by-step guide](https://ay-obami.github.io/ShieldSpec/docs.html) provides actual CLI commands and expected outcomes. The browser is a **read-only explainer and evidence explorer**; it does not run a wallet or a live fuzzing campaign. The website source (`site/`) and historical evidence (`evidence/historical-zallet-677/`) are included in `main`.

Choose the workflow that fits your integration:

| Need | Entry point | What it tests |
| --- | --- | --- |
| Check a proposal snapshot or CI fixture | `shieldspec-cli check` / `diff` or the GitHub Action | Explicit invariants on normalized proposal facts |
| Exercise real SDK proposals locally | `shieldspec-fuzz fuzz` with `shieldspec-zcash-runner` | Deterministic synthetic wallet states and proposal construction |
| Compare wallet/SDK revisions | `shieldspec-fuzz fuzz` with **two independently built** compatible runners | New/worsened findings and comparable named wallet-policy decisions |
| Audit a recorded difference | `shieldspec-fuzz replay --input <discovery.json>` | Saved scenario, runner hashes, arguments, and observed result |

For fewer flags, start with the published [CLI v0.4.0 `shieldspec try` and configured `shieldspec test` workflows](docs/simple-workflows.md). The advanced `check`, `diff`, and `fixture` commands remain available. The examples below remain the stable advanced interface; consult the [getting-started guide](docs/getting-started.md) for real-runner setup.

## Quickstart: the snapshot privacy gate

For the published CLI (Rust 1.95+), install version `0.4.0` from crates.io:

```bash
cargo install shieldspec-cli --version 0.4.0 --locked
shieldspec try
```

The installed executable is `shieldspec`. `shieldspec try` runs a bundled snapshot negative control (not a wallet test); `shieldspec test` reads `shieldspec.toml`. The commands below demonstrate the advanced interface against pinned repository fixtures.

With Git, Rust 1.95.0 (pinned in `rust-toolchain.toml`), and the repository cloned:

```bash
git clone https://github.com/Ay-obami/ShieldSpec.git
cd ShieldSpec

cargo run --locked -p shieldspec-cli -- \
  check --input fixtures/shielded-payment.json \
  --policy standard --format human

cargo run --locked -p shieldspec-cli -- \
  diff --baseline fixtures/shielded-payment.json \
  --candidate fixtures/transparent-change.json \
  --policy standard --format human
```

The first command passes. The second reports `SS003_TRANSPARENT_CHANGE` and **intentionally exits 2**. The lightweight CLI also supports JSON and SARIF; use `--policy strict` for stricter snapshot invariants.

Exit statuses for the CLI and differential fuzzer:

| Code | Meaning |
| --- | --- |
| `0` | Completed without a configured failing finding or differential difference |
| `1` | Input, runner, protocol, or other operational error; **not** a valid privacy verdict |
| `2` | Snapshot policy failure, new/worsened privacy finding, **or** a comparable wallet-policy decision difference |

A code 2 from a **wallet-policy decision difference** does not automatically mean a privacy regression; inspect the named policy and both verdicts.

## Quickstart: real SDK differential fuzzing (on `main`)

From the same clone, build the two real-wallet binaries. No PR checkout or historical reproduction setup is required:

```bash
cargo build --locked -p shieldspec-zcash-runner --bins
```

These commands exercise deterministic wallet state through the pinned Zcash SDK in local synthetic chains. A runner compared with itself is a **negative control**, not evidence that two wallet implementations agree or that a privacy defect has been found.

A snapshot-only real-wallet self-comparison (the runner is **not** instrumented for a wallet-policy verdict in this mode):

```bash
target/debug/shieldspec-fuzz fuzz \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --seed 148 --cases 2 --format human
```

Expect `no_regression_found`. In JSON output, all wallet-policy observation counts are `0` because neither runner was started with `--wallet-spend-policy`. This verifies only a snapshot differential control, not wallet-policy agreement.

To exercise **four real SDK NU6.3 proposals** and both sides' real Orchard-only spend-policy observations:

```bash
target/debug/shieldspec-fuzz fuzz \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --baseline-arg=--wallet-spend-policy --baseline-arg=orchard-only \
  --candidate-arg=--wallet-spend-policy --candidate-arg=orchard-only \
  --scenario-family ironwood --seed 2026 --cases 4 --format json
```

The four ordinals cover:

| Ordinal | Funding | NU6.3 | Real Orchard-only spend verdict |
| --- | --- | --- | --- |
| `0` | Orchard | At activation | Accepted |
| `1` | Orchard | Before activation | Accepted |
| `2` | Sapling | At activation | Rejected |
| `3` | Sapling | Before activation | Rejected |

For identical runners, the expected campaign status is `no_regression_found` with all three wallet-policy observation counts (`baseline`, `candidate`, `comparable`) equal to `4`. This does **not** mean there were no per-proposal warnings: at-activation proposals can contain actual Orchard→Ironwood or Sapling→Ironwood cross-pool value balances flagged by `SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE` on *both* sides. The real SDK `zcash_spend_orchard_only_v1` policy **restricts spend-source pools**; it is not Zallet `FullPrivacy`.

Inspect an individual case with the same runner settings:

```bash
target/debug/shieldspec-fuzz replay \
  --scenario-family ironwood --seed 2026 --ordinal 2 \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --baseline-arg=--wallet-spend-policy --baseline-arg=orchard-only \
  --candidate-arg=--wallet-spend-policy --candidate-arg=orchard-only \
  --format json
```

PR #43 also adds real Sapling-recipient proposals, mixed Unified Address receiver selection, transparent-UTXO-funded shielded payments, fee and change accounting, fragmented-note selection, and mixed Sapling/Orchard input selection. These are **SDK-backed scenario and test coverage**, not claimed newly discovered privacy vulnerabilities. The ordinary generator retains its established replay sequence; transparent-funding cases are opt-in:

```bash
target/debug/shieldspec-fuzz fuzz \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --transparent-funding --seed 7523070 --cases 48 --format json
```

For a seed/ordinal replay from that opt-in stream, use `replay --transparent-funding --seed ... --ordinal ...` with the same runner options. Do not combine `--transparent-funding` with `--scenario-family ironwood`; those are distinct generators. Test the real-wallet paths with:

```bash
cargo test --locked -p shieldspec-zcash-runner --all-targets
```

For genuine differential testing, compile **independent compatible baseline and candidate runners** against the wallet/SDK revisions you intend to compare. The tool passes the same scenario JSON to both and compares normalized `ProposalSnapshot` findings **and, only when observed, the same named wallet-policy verdict**. It reports operational errors separately.

## How it works

The real-proposal harness uses isolated in-memory Zcash wallet state, scans synthetic compact blocks through wallet SDK code, invokes real proposal selection, and converts actual `Proposal` values into a stable snapshot. The fixtures and chain are synthetic; SDK selection, fees, change, steps, and routing are not made-up proposal JSON. Opt-in `orchard-only` observation calls the SDK's independent restricted spend-source policy while retaining the **unrestricted** proposal as a diagnostic snapshot; an SDK insufficient-allowed-funds result is a normal rejection, while operational errors remain errors. A ZIP 320 TEX case preserves its two-step internal transparent bridge provenance, so an internal bridge is not mislabeled as wallet transparent change or funding.

Separate runner processes allow two builds to compare without sharing librustzcash Rust types:

1. Generate a deterministic scenario by seed, ordinal, and scenario family (`legacy`, `ironwood`, `zallet-privacy`, or opt-in `transparent`).
2. Ask baseline and candidate runners for real proposal snapshots and optional first-class policy verdicts.
3. Evaluate proposal rules such as `SS003`, `SS005`, and `SS006`, plus comparable observed policy decisions.
4. On a difference, semantically shrink the case and record a replayable result.

`fuzz --save <path>` saves a **discovery only when a difference is found and the original and minimized findings revalidate**, including the original and minimized scenarios, executable SHA-256, exact runner arguments, and optional *declared* `--baseline-sdk`/`--candidate-sdk` labels. `replay --input <path>` checks binary hashes and argument identities before reproducing. A hash identifies executable bytes; a user-declared SDK label does not prove the source revision. Seed/ordinal replay for the Ironwood family must include `--scenario-family ironwood`.

See [the runner protocol and replay outcomes](docs/wallet-policy-observations.md) and [the committed replay corpus](crates/shieldspec-zcash-runner/fixtures/discoveries/).

## Evidence versus discovery

[Historical Zallet #677](evidence/historical-zallet-677/README.md) is a separate, pinned source-level witness: affected `f496c9b` accepted an Ironwood→Sapling crossing under its actual `FullPrivacy` policy; corrected `e966780` rejected it. This was **previously documented and fixed upstream**. The original fixed witness is not a new vulnerability. A **separate, now verified automatic integration** exercises both independently compiled historical wallet-policy implementations through ShieldSpec's generic fuzzer: seed 2026 finds a named verdict difference at ordinal 2 after two agreeing controls, minimizes the case, and reproduces it with verified SHA-linked runner provenance. See [the complete live-run recipe](docs/zallet-auto-discovery.md). Reproduction requires Rust 1.95, network access, and native upstream build dependencies:

```bash
python3 scripts/reproduce_zallet_677.py --output evidence/historical-zallet-677/results
```

The distinct [librustzcash #2614 evidence](evidence/historical-2614/README.md) concerns a transparent send-max **correctness** difference, not a demonstrated privacy leak.

A separately available `shieldspec-fuzz demo` subcommand deliberately injects transparent change in a candidate to exercise detection, shrinking, and replay. It is a **controlled fixture**, not upstream behavior or a claimed discovered vulnerability. See [the getting-started guide](docs/getting-started.md) for the optional command.

## Integrate the lightweight GitHub Action

In a consumer repository with normalized snapshot fixtures:

```yaml
- name: Check proposal privacy regression
  uses: Ay-obami/ShieldSpec@v0.2.0
  with:
    baseline: fixtures/shielded-payment.json
    candidate: fixtures/transparent-change.json
    policy: standard
    format: sarif
```

The Action runs the snapshot CLI only; it does **not** build or fuzz a wallet. It exits with the same 0/1/2 statuses and exposes a `status` output. This example intentionally fails on the committed transparent-change fixture; use your own expected baseline/candidate files.

The `v0.2.0` source tag includes the expanded differential fuzzer but its composite Action runs **only** the snapshot check. For differential CI, use the newer [Action guidance](docs/github-action.md); the `v0.4.0` CLI release adds simpler `try` and `test` commands. The underlying differential runner binaries still require separate setup. Release tags are immutable and do not track the moving `main` branch.

The **`v0.3.0` GitHub Action/source release** adds opt-in differential CI while keeping snapshot mode as the default. For a compact installation guide and accurate feature boundaries, see [GitHub Action setup](docs/github-action.md). It provides a first-party `zcash-sdk` **negative-control** preset for quick exploration and `custom` runners built by wallet developers for actual implementation comparisons:

```yaml
- name: Explore ShieldSpec SDK differential CI
  uses: Ay-obami/ShieldSpec@v0.3.0
  with:
    mode: differential
    preset: zcash-sdk
    seed: '148'
    cases: '2'
```

This preset compares the same pinned SDK runner against itself; it is **not** a test of your wallet or a cross-version discovery. Teams adding their own wallet can begin with the [Rust runner-adapter starter](examples/runner-adapter/README.md), [protocol preflight](examples/runner-adapter/README.md#try-it), and [custom runner and artifact integration](docs/ci-integration.md#differential-action-custom-wallet-runners). The starter's `--demo` mode produces synthetic snapshots only. The `v0.3.0` tag introduced the differential GitHub Action. `shieldspec-cli` is now published at `0.4.0`; the separate library crates remain `0.2.0`.

## Project layout and development

- `shieldspec-core` — normalized proposal model, privacy rules, and comparisons.
- `shieldspec-testkit` — builders and canonical snapshot fixtures.
- `shieldspec-zcash` — adapter for real Zcash SDK proposals.
- `shieldspec-fuzz` — SDK-independent two-process runner protocol and differential comparison.
- `shieldspec-zcash-runner` — isolated real-wallet fixture harness, scenario generation, shrink/replay, and `shieldspec-fuzz` binary.
- `shieldspec-cli` and `action.yml` — fast snapshot privacy gate.
- `examples/mock-wallet` — independent downstream consumption example.

The repository pins Rust 1.95.0. Maintainers can verify the complete workspace with `cargo fmt --check`, `cargo test --locked --workspace --all-targets`, and `cargo clippy --locked --workspace --all-targets -- -D warnings`. For a narrower check of the real-wallet tooling, run `cargo test --locked -p shieldspec-fuzz -p shieldspec-zcash-runner --all-targets`. Historical upstream compilation remains opt-in for local development; a separate [automatic Zallet discovery workflow](.github/workflows/zallet-auto-discovery.yml) tests both pinned revisions in CI.

**Scope:** no real signing, broadcast, user seed phrases, custody, anonymity score, or universal privacy guarantee. See [privacy limitations](docs/privacy-model.md).

## Project history

ShieldSpec was demonstrated in the 2026 Zecathon Core & Tooling hackathon. The [archived submission notes](docs/submission.md) and [recording storyboard](docs/demo-video-script.md) document that milestone; the maintained project is the developer tool, its tests, and its reproducible source-level evidence.
