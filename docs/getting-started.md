# Get started with ShieldSpec

ShieldSpec is a local, pre-signing privacy-regression tool for Zcash proposal snapshots. The fast snapshot CLI works on `main`; the generic real-wallet policy-verdict comparison and four-case NU6.3 generator are currently in [open PR #42](https://github.com/Ay-obami/ShieldSpec/pull/42), not yet merged as of 2026-10-27.

You will first check committed fixtures, then (optionally) compile the real Zcash SDK runner and compare observed wallet-policy decisions. **Neither workflow signs or broadcasts transactions, and success is not an anonymity guarantee.**

## Prerequisites and checkout

- Git and the pinned **Rust 1.95.0** toolchain (`rust-toolchain.toml`, including rustfmt and clippy).
- Disk space and memory for compiling Rust dependencies; the real Zcash runner is much heavier than the snapshot CLI.
- Python 3 only for site preview and optional historical-source reproductions.
- Network access for the initial clone/dependency fetch. Upstream historical reproduction has additional native build dependencies.

```bash
git clone https://github.com/Ay-obami/ShieldSpec.git
cd ShieldSpec
rustc --version
```

The snapshot steps below work on the default `main` checkout. To follow the later **PR #42 preview** commands before they merge, switch to its head:

```bash
git fetch origin pull/42/head
git switch --detach FETCH_HEAD
```

This deliberately checks out the proposed revision in detached-HEAD mode. Once the feature is on your target branch, remain on that branch instead. Check the branch or commit before attributing behavior to a published release.

## 1. Check and compare snapshots

```bash
cargo run --locked -p shieldspec-cli -- \
  check --input fixtures/shielded-payment.json \
  --policy standard --format human
```

The clean fixture produces a policy pass and exits `0`. Compare it with transparent change:

```bash
cargo run --locked -p shieldspec-cli -- \
  diff --baseline fixtures/shielded-payment.json \
  --candidate fixtures/transparent-change.json \
  --policy standard --format human
```

Expect the new finding `SS003_TRANSPARENT_CHANGE` and exit `2`. This is a *known fixture*, not a new Zcash vulnerability. Select `--format json` or `--format sarif` for machine-readable snapshot reports.

| Exit | Snapshot CLI | Differential fuzzer |
| --- | --- | --- |
| `0` | No configured failing finding/regression | No new configured snapshot regression **and** no comparable wallet-policy decision difference |
| `1` | Invalid input/operational error | Runner/protocol/construction error; no valid verdict |
| `2` | Configured finding/regression | New/worsened snapshot finding **or** changed actual named wallet-policy decision |

**Do not equate every fuzzer exit 2 with a leak.** A policy decision change may reflect a stricter wallet, config change, or bug fix; review the contract and both verdicts.

## 2. Run a real SDK differential campaign (PR #42)

After checking out a revision containing PR #42:

```bash
cargo build --locked -p shieldspec-zcash-runner --bins
target/debug/shieldspec-fuzz fuzz \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --seed 148 --cases 2 --format human
```

Both runner paths are intentionally identical: this is a no-difference control, expected to exit `0`. The runner creates synthetic, isolated wallet state, scans compact blocks with the real SDK, executes real proposal construction, and emits a normalized `ProposalSnapshot`. There is no mainnet wallet or seed phrase.

Now run the four-case **NU6.3/Ironwood** scenario family with opt-in real Orchard-only spend-policy observations:

```bash
target/debug/shieldspec-fuzz fuzz \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --baseline-arg=--wallet-spend-policy --baseline-arg=orchard-only \
  --candidate-arg=--wallet-spend-policy --candidate-arg=orchard-only \
  --scenario-family ironwood --seed 2026 --cases 4 --format json
```

The generator assigns funding/activation states by ordinal:

| Ordinal | Funding pool | NU6.3 state | SDK `zcash_spend_orchard_only_v1` |
| --- | --- | --- | --- |
| 0 | Orchard | At activation | `accepted` |
| 1 | Orchard | Before activation | `accepted` |
| 2 | Sapling | At activation | `rejected` |
| 3 | Sapling | Before activation | `rejected` |

Because both runner executables and policy arguments match, expect `status: "no_regression_found"`, `cases_executed: 4`, and all three policy coverage counts at `4` (baseline, candidate, comparable). **Both sides agreeing on rejections is not an operational error.** These are *normal SDK spend-source policy rejections*.

For one case and its actual verdicts, replay ordinal 2:

```bash
target/debug/shieldspec-fuzz replay \
  --scenario-family ironwood --seed 2026 --ordinal 2 \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --baseline-arg=--wallet-spend-policy --baseline-arg=orchard-only \
  --candidate-arg=--wallet-spend-policy --candidate-arg=orchard-only \
  --format json
```

At activation, the actual SDK may route the recipient into Ironwood even though the source wallet is funded in Orchard or Sapling. ShieldSpec's `SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE` describes the proposal's publicly visible cross-pool net balance. It **does not** mean the separate named Orchard-only spend-policy verdict is `rejected`; that verdict depends on *source-pool restriction*, not a Zallet `FullPrivacy` policy. Snapshot findings present in *both* runners do not by themselves constitute a differential regression.

The activation-aware family is opt-in; ordinary seed/ordinal replay remains the legacy generator. When replaying by seed and ordinal, keep `--scenario-family ironwood` to select the same generated case.

## 3. Compare different wallet builds and preserve a finding

Replace the two runner paths with **independently compiled, protocol-compatible** executables built against the SDK/wallet revisions you want to test. Give both the exact same policy name and instrument its real enforcement; a verdict must not be guessed from a snapshot.

```bash
target/debug/shieldspec-fuzz fuzz \
  --baseline-runner /path/to/baseline-runner \
  --candidate-runner /path/to/candidate-runner \
  --seed 148 --cases 25 --policy standard \
  --save /path/to/discovery.json --format json
```

This command is a **template**: substitute real executable paths and choose the appropriate runner args. `--save` creates a file **only when a stable differential finding is detected**, shrunk, and rechecked. Its artifact includes executable hashes and exact arguments; optional `--baseline-sdk`/`--candidate-sdk` fields are *declared* labels, not verification of the source revision.

Reproduce a saved finding:

```bash
target/debug/shieldspec-fuzz replay \
  --input /path/to/discovery.json --format json
```

Replay verifies recorded runner bytes, arguments, and policy before comparing the result. Supply explicit runner paths only when moving the discovery to equivalent binaries; changed hashes or conflicting arguments fail closed. A changed decision direction does not count as reproducing the original policy observation.

See [wallet-policy protocol and result semantics](wallet-policy-observations.md), including the difference between a real rejection and an operational failure.

## 4. Add the lightweight GitHub Action to CI

For a repository with normalized baseline and candidate snapshot files:

```yaml
- name: Check proposal privacy regression
  uses: Ay-obami/ShieldSpec@v0.1.0
  with:
    baseline: fixtures/shielded-payment.json
    candidate: fixtures/transparent-change.json
    policy: standard
    format: sarif
```

The Action checks snapshots and fails the job for exit `1` or `2`; its `status` output carries the same code. Replace the example paths with your own. The example's transparent-change candidate **intentionally fails**, so it is not a copy-paste passing production gate. The Action does **not** build a wallet or run fuzzing.

## 5. Optional evidence and local website

The public website is reachable independently, but the local `site/` and `evidence/historical-zallet-677/` folders are currently in the feature checkout, **not** the local `main` Git tree. Switch to the PR #42 head from the prerequisites before running the following local preview or historical command.

- [Live product website](https://ay-obami.github.io/ShieldSpec/) (HTTP 200 verified 2026-10-27), with a read-only curated historical-evidence explorer.
- Local preview:

```bash
python3 -m http.server 8765 --directory site
```

Open `http://127.0.0.1:8765`. Serve over HTTP so the case explorer can fetch JSON.

- [Zallet #677 historical witness](../evidence/historical-zallet-677/README.md): the fixed, previously documented Ironwood→Sapling `FullPrivacy` policy behavior. Optional reproduction requires Python 3, network access, disk space, and upstream build dependencies:

```bash
python3 scripts/reproduce_zallet_677.py \
  --output evidence/historical-zallet-677/results
```

This pinned historical policy comparison is **not yet automatic** in the general fuzzer. Generic verdict detection is shipped in PR #42; a real Zallet `FullPrivacy` runner pair and general automatic historical discovery remain future integration work. The [separate librustzcash #2614 case](../evidence/historical-2614/README.md) is SDK correctness history, not a proven privacy leak.

- An **intentionally injected** transparent-change demo remains available for checking detection/minimization/replay without claiming an upstream discovery:

```bash
cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- \
  demo --format human
```

That controlled demo's **exit code 2** is expected. It is not needed for normal onboarding.

Next read the [privacy model](privacy-model.md) and [roadmap](roadmap.md), or open a focused issue with a reproducible scenario and clear policy expectation.