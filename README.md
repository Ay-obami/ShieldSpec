# ShieldSpec

**Privacy regression testing for Zcash wallet transaction proposals.** ShieldSpec checks normalized proposal facts against explicit invariants, compares baseline and candidate behavior, and—on the current development branch—drives real Zcash SDK proposal construction through deterministic differential fuzzing.

ShieldSpec is developer tooling, not a wallet, anonymity score, signer, or transaction broadcaster. A passing check means only that the configured properties held for the inputs exercised.

- [Get started](docs/getting-started.md): commands, expected outputs, exit codes, and CI integration
- [Privacy model](docs/privacy-model.md): what snapshots and actual wallet-policy verdicts do—and do not—establish
- [Wallet-policy observations](docs/wallet-policy-observations.md): runner protocol, real SDK Orchard-only mode, and NU6.3 campaign
- [Roadmap](docs/roadmap.md): shipped features versus integration and future work
- [Live developer website](https://ay-obami.github.io/ShieldSpec/) · [site source and local preview](site/README.md)

The GitHub Pages URL has been checked to return HTTP 200 (2026-10-27). Its historical-evidence explorer is read-only; **the browser does not run a wallet or live fuzzing**. The local `main` Git tree does not yet contain `site/` or `evidence/historical-zallet-677/`; use the PR #42 feature checkout below for those paths until integrated.

## Start with the snapshot privacy gate (available on main)

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

## Real SDK fuzzing and policy observations (PR #42 preview)

The generic wallet-policy comparison and activation-aware Ironwood scenario family are implemented in [PR #42](https://github.com/Ay-obami/ShieldSpec/pull/42), which is **open and not merged into `main` as of 2026-10-27**. To run these commands from a fresh clone today, fetch its current head first:

```bash
git fetch origin pull/42/head
git switch --detach FETCH_HEAD
cargo build --locked -p shieldspec-zcash-runner --bins
```

The detached checkout is intentional for testing a specific proposed revision. Skip the fetch/switch once those features have reached the branch you are using.

A clean real-wallet self-comparison:

```bash
target/debug/shieldspec-fuzz fuzz \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --seed 148 --cases 2 --format human
```

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

For identical runners, the expected campaign status is `no_regression_found` with `comparable_wallet_policy_cases: 4`. This does **not** mean there were no per-proposal warnings: at-activation proposals can contain actual Orchard→Ironwood or Sapling→Ironwood cross-pool value balances flagged by `SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE` on *both* sides. The real SDK `zcash_spend_orchard_only_v1` policy **restricts spend-source pools**; it is not Zallet `FullPrivacy`.

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

For genuine differential testing, compile **independent compatible baseline and candidate runners** against the wallet/SDK revisions you intend to compare. The tool passes the same scenario JSON to both and compares normalized `ProposalSnapshot` findings **and, only when observed, the same named wallet-policy verdict**. It reports operational errors separately.

## How it works

The real-proposal harness uses isolated in-memory Zcash wallet state, scans synthetic compact blocks through wallet SDK code, invokes `propose_standard_transfer`, and converts actual `Proposal` values into a stable snapshot. The fixtures and chain are synthetic; SDK selection, fees, change, steps, and routing are not made-up proposal JSON. A ZIP 320 TEX case preserves its two-step internal transparent bridge provenance, so an internal bridge is not mislabeled as wallet transparent change or funding.

Separate runner processes allow two builds to compare without sharing librustzcash Rust types:

1. Generate a deterministic scenario by seed, ordinal, and scenario family.
2. Ask baseline and candidate runners for real proposal snapshots and optional first-class policy verdicts.
3. Evaluate proposal rules such as `SS003`, `SS005`, and `SS006`, plus comparable observed policy decisions.
4. On a difference, semantically shrink the case and record a replayable result.

`fuzz --save <path>` saves a **discovery only when a difference is found and preserved**, including the minimized scenario, executable SHA-256, exact runner arguments, and optional *declared* `--baseline-sdk`/`--candidate-sdk` labels. `replay --input <path>` checks binary hashes and argument identities before reproducing. A hash identifies executable bytes; a user-declared SDK label does not prove the source revision. Seed/ordinal replay for the Ironwood family must include `--scenario-family ironwood`.

See [the runner protocol and replay outcomes](docs/wallet-policy-observations.md) and [the committed replay corpus](crates/shieldspec-zcash-runner/fixtures/discoveries/).

## Evidence versus discovery

[Historical Zallet #677](evidence/historical-zallet-677/README.md) is a separate, pinned source-level witness: affected `f496c9b` accepted an Ironwood→Sapling crossing under its actual `FullPrivacy` policy; corrected `e966780` rejected it. This was **previously documented and fixed upstream**. The witness and control cases are not a new vulnerability or an automatically discovered generic-fuzzer verdict difference. Reproduction requires Rust 1.95, network access, and native upstream build dependencies:

```bash
python3 scripts/reproduce_zallet_677.py --output evidence/historical-zallet-677/results
```

The distinct [librustzcash #2614 evidence](evidence/historical-2614/README.md) concerns a transparent send-max **correctness** difference, not a demonstrated privacy leak.

A separately available `shieldspec-fuzz demo` subcommand deliberately injects transparent change in a candidate to exercise detection, shrinking, and replay. It is a **controlled fixture**, not upstream behavior or a claimed discovered vulnerability. See [the getting-started guide](docs/getting-started.md) for the optional command.

## Integrate the lightweight GitHub Action

In a consumer repository with normalized snapshot fixtures:

```yaml
- name: Check proposal privacy regression
  uses: Ay-obami/ShieldSpec@v0.1.0
  with:
    baseline: fixtures/shielded-payment.json
    candidate: fixtures/transparent-change.json
    policy: standard
    format: sarif
```

The Action runs the snapshot CLI only; it does **not** build or fuzz a wallet. It exits with the same 0/1/2 statuses and exposes a `status` output. This example intentionally fails on the committed transparent-change fixture; use your own expected baseline/candidate files.

## Project layout and development

- `shieldspec-core` — normalized proposal model, privacy rules, and comparisons.
- `shieldspec-testkit` — builders and canonical snapshot fixtures.
- `shieldspec-zcash` — adapter for real Zcash SDK proposals.
- `shieldspec-fuzz` — SDK-independent two-process runner protocol and differential comparison.
- `shieldspec-zcash-runner` — isolated real-wallet fixture harness, scenario generation, shrink/replay, and `shieldspec-fuzz` binary.
- `shieldspec-cli` and `action.yml` — fast snapshot privacy gate.
- `examples/mock-wallet` — independent downstream consumption example.

The repository pins Rust 1.95.0. Maintainers can verify the complete workspace with `cargo fmt --check`, `cargo test --locked --workspace --all-targets`, and `cargo clippy --locked --workspace --all-targets -- -D warnings`. For a narrower check of the real-wallet tooling, run `cargo test --locked -p shieldspec-fuzz -p shieldspec-zcash-runner --all-targets`. Historical upstream reproduction is opt-in, not part of normal CI.

**Scope:** no real signing, broadcast, user seed phrases, custody, anonymity score, or universal privacy guarantee. See [privacy limitations](docs/privacy-model.md).