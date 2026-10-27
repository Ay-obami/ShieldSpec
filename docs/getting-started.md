# Get started with ShieldSpec

ShieldSpec is an open-source privacy-regression testkit for Zcash wallet transaction proposals. It can assess normalized proposal snapshots, compare them against a privacy baseline, run a deterministic **real-wallet proposal** fuzz campaign, and replay minimized cases.

> **Important:** ShieldSpec is a test instrument, not a privacy guarantee, blockchain explorer, wallet, signer, or live transaction broadcaster. The browser landing page displays verified recorded cases; it does not run fuzzing.

## Requirements

- Git and a Rust toolchain. The repository pins **Rust 1.95.0** with `rustfmt` and `clippy` in `rust-toolchain.toml`.
- Enough memory and storage to build the Zcash SDK dependencies. Normal CLI snapshot comparison is much cheaper than historical upstream reproduction.
- Python 3 for the optional historical scripts. Reproducing the full historical Zallet fixture also needs network access and its upstream native build dependencies.

```bash
git clone https://github.com/Ay-obami/ShieldSpec.git
cd ShieldSpec
cargo test --locked --workspace --all-targets
```

## 1. Check a proposal snapshot

A snapshot records input pools, output roles and amounts, fee, and step provenance. Use a committed fixture first:

```bash
cargo run --locked -p shieldspec-cli -- \
  check \
  --input fixtures/shielded-payment.json \
  --policy standard \
  --format human
```

To compare privacy behavior between two normalized proposals:

```bash
cargo run --locked -p shieldspec-cli -- \
  diff \
  --baseline fixtures/shielded-payment.json \
  --candidate fixtures/transparent-change.json \
  --policy standard \
  --format human
```

This second command reports a **transparent-change privacy regression** (`SS003_TRANSPARENT_CHANGE`).

The command exit statuses are part of the contract:

| Status | Meaning |
| --- | --- |
| **0** | Check completed without a policy failure or new differential finding |
| **1** | Operational/input error; no valid privacy verdict |
| **2** | Privacy finding or regression according to the selected check |

A status of 0 means only the configured properties held for the tested data. It cannot prove anonymity or absence of all leaks.

## 2. Run the realistic deterministic fuzzer demo

ShieldSpec has a real Zcash proposal runner: it creates isolated in-memory wallet state, scans synthetic compact blocks, invokes Zcash proposal construction, and normalizes the result.

```bash
cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- \
  demo --format human
```

The quick demo includes an **intentionally injected** transparent-change fault in the candidate. Its **exit code 2 is expected** and demonstrates the differential reporting, minimization, and replay pipeline. It does **not** represent an upstream vulnerability automatically discovered by ShieldSpec.

Replay that controlled case:

```bash
cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- \
  replay --seed 148 --ordinal 0 --policy standard \
  --demo-regression transparent-change --format human
```

For an unfaulted self-comparison, build the binaries once:

```bash
cargo build --locked -p shieldspec-zcash-runner --bins
target/debug/shieldspec-fuzz fuzz \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --seed 148 --cases 2 --format human
```

The identical runner on both sides should not introduce a differential privacy regression. A broader campaign uses separate real runner executables compiled against the intended wallet/SDK revisions. Labels supplied through `--baseline-sdk` and `--candidate-sdk` are declared context, **not** proof of source revision. Saved runner executable hashes are checked on replay.

## 3. Reproduce documented Zcash privacy history

ShieldSpec includes a verified historical reproduction of [zcash/zallet#677](https://github.com/zcash/zallet/pull/677). The pinned **affected** wallet-core policy accepted an Ironwood→Sapling FullPrivacy case while the **corrected** version rejected it. The same fixture has control cases to distinguish that difference from ordinary wallet behavior.

Read [the exact historical evidence and raw logs](../evidence/historical-zallet-677/README.md) before rebuilding. To rerun it, when you have time, bandwidth, disk space and build dependencies:

```bash
python3 scripts/reproduce_zallet_677.py \
  --output evidence/historical-zallet-677/results
```

This is a **previously documented and fixed upstream privacy-policy behavior**, not a new security disclosure. Its test-only witness runs real Zallet code. In the currently published implementation, that historical proof is **not yet automatically discovered by the general ShieldSpec fuzzer**; generalizing that discovery is an active roadmap item.

See [the other SDK correctness reproduction](../evidence/historical-2614/README.md) for a distinct example that must not be mislabeled a privacy leak.

## 4. GitHub Action integration

In a consuming repository with your own snapshot fixtures:

```yaml
- name: Check proposal privacy regression
  uses: Ay-obami/ShieldSpec@v0.1.0
  with:
    baseline: fixtures/shielded-payment.json
    candidate: fixtures/transparent-change.json
    policy: standard
    format: sarif
```

This lightweight GitHub Action checks snapshots; it does not automatically set up or fuzz a live wallet. Its `status` output uses the same 0/1/2 meanings. Customize the candidate/baseline paths to your application and treat a privacy failure as a failing CI gate.

## 5. Explore the product website

The static website lives at [`site/`](../site/README.md). To preview it locally:

```bash
python3 -m http.server 8765 --directory site
```

Open `http://127.0.0.1:8765`. The interactive explorer displays **recorded and labeled** test results; the actual harness runs only through Rust/Python tools.

## Next steps

- Review [the privacy model and test limitations](privacy-model.md).
- Track [the feature roadmap](roadmap.md) and [open GitHub issues](https://github.com/Ay-obami/ShieldSpec/issues).
- Use the [reproducible demo walkthrough](demo.md) or open a focused issue for a missing wallet scenario.
