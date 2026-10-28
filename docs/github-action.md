# ShieldSpec GitHub Action — install and use

ShieldSpec is a pre-signing Zcash proposal privacy-regression tool. Its composite GitHub Action is published as source at [`Ay-obami/ShieldSpec@v0.3.0`](https://github.com/Ay-obami/ShieldSpec/releases/tag/v0.3.0). The four crates.io packages are separately versioned at `0.2.0`. The newer Rust runner-adapter starter is on `main`, **not** in the immutable `v0.3.0` tag.

## Quick start: snapshot privacy gate

Add this step to a Linux GitHub Actions job after checking out your repository and producing a normalized proposal snapshot:

```yaml
- name: Check proposal privacy
  uses: Ay-obami/ShieldSpec@v0.3.0
  with:
    mode: snapshot
    candidate: fixtures/proposal.json
    policy: standard
    format: human
```

Replace `fixtures/proposal.json` with a file in your repository that conforms to ShieldSpec's [snapshot schema](../fixtures/shielded-payment.json). Optional `baseline` compares snapshots instead of checking one. Use an immutable full commit SHA for tighter supply-chain pinning.

## Differential mode: SDK negative control

```yaml
- name: Exercise differential infrastructure
  uses: Ay-obami/ShieldSpec@v0.3.0
  with:
    mode: differential
    preset: zcash-sdk
    seed: '148'
    cases: '2'
```

This compiles and compares **the same pinned SDK runner against itself**. It confirms the Action can execute a deterministic differential campaign; it does **not** inspect your wallet or find a new privacy vulnerability.

## Differential mode: your wallet

Build two independently compiled, protocol-compatible executables before the Action step:

```yaml
- name: Compare wallet revisions
  uses: Ay-obami/ShieldSpec@v0.3.0
  with:
    mode: differential
    preset: custom
    baseline-runner: target/baseline/wallet-runner
    candidate-runner: target/candidate/wallet-runner
    cases: '25'
    seed: '2026'
    artifact-name: shieldspec-wallet-evidence
```

The paths point to executable files in the caller's workspace. The Action does not clone, instrument, or compile arbitrary third-party wallets. See the [Rust-only runner starter](../examples/runner-adapter/README.md) on `main`, its [protocol validator](../crates/shieldspec-fuzz/src/bin/shieldspec-validate-runner.rs), and the [runner contract](runner-protocol-contract.md). The starter's `--demo` produces a fixed synthetic snapshot and is a protocol control, not a real wallet test.

## Outputs, guarantees and constraints

- Status `0`: no configured failing finding or differential regression in exercised scenarios; **not** proof of anonymity.
- Status `1`: operational, parsing or runner failure.
- Status `2`: configured privacy finding or differential behavior requiring investigation, **not** automatically a vulnerability.
- Differential mode uploads JSON campaign reports and minimized discovery evidence when present. Snapshot `sarif` is printed to the log, **not** automatically uploaded to GitHub code scanning.
- Pin Rust 1.95.0 and the tested Zcash SDK prerelease dependencies when reproducing source evidence. No signing, broadcasting or custody is performed.

For a complete CI example, see [CI integration](ci-integration.md) and the repository's [Action demo workflow](../.github/workflows/shieldspec-demo.yml).
