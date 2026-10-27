# ShieldSpec

ShieldSpec is privacy regression testing for Zcash wallet transaction proposals. It lets developers turn privacy expectations into tests and CI failures before wallet changes ship.

ShieldSpec is a **Core & Tooling** project: a reusable Rust policy engine and testkit sit beneath a CLI, Zcash proposal adapter, and GitHub Action. It inspects proposal facts available before signing; it does not handle wallet secrets or claim that a clean result guarantees anonymity.

## Five-minute demo

You need Rust 1.95 or newer; this repository pins Rust 1.95.0 for reproducible verification. From its root, compare the canonical shielded baseline with a candidate that adds transparent change:

```bash
cargo run -p shieldspec-cli -- \
  diff \
  --baseline fixtures/shielded-payment.json \
  --candidate fixtures/transparent-change.json \
  --policy standard \
  --format human
```

The command reports `SS003_TRANSPARENT_CHANGE` and exits with status `2`. The recipient remains shielded in both snapshots; inspecting only the destination would miss the regression.

For a downstream-library version of the same moment:

```bash
cargo test --manifest-path examples/mock-wallet/Cargo.toml
cargo run --manifest-path examples/mock-wallet/Cargo.toml
```

The test first proves the baseline passes structurally, then proves `compare()` fails and includes `SS003_TRANSPARENT_CHANGE`. The executable also exits `2`, making the result directly usable as a CI gate.

## GitHub Action

Commit or generate proposal snapshots without keys, then add a workflow step:

```yaml
- name: Check proposal privacy regression
  uses: Ay-obami/ShieldSpec@v0.1.0
  with:
    baseline: fixtures/shielded-payment.json
    candidate: fixtures/transparent-change.json
    policy: standard
    format: sarif
```

Omit `baseline` to check one candidate against the selected policy. The action's `status` output is the CLI exit status: `0` for a pass, `1` for an operational error, or `2` for a privacy failure. Pin a specific release tag (for example, `v0.1.0`) rather than a mutable branch. The action requires no wallet seed, spending key, signing access, or network credentials. It does require outbound network access to install the pinned Rust toolchain and download Cargo dependencies unless those assets are already available in the runner's caches.

## Architecture

- `shieldspec-core` defines normalized proposal snapshots, policy findings, assessment, and structural regression comparison.
- `shieldspec-testkit` supplies ergonomic builders and canonical privacy fixtures.
- `shieldspec-zcash` converts supported Zcash SDK proposal structures into normalized snapshots.
- `shieldspec-cli` exposes `check`, `diff`, and fixture commands with human, JSON, and SARIF output.
- `action.yml` runs the CLI as a reusable composite GitHub Action.
- `examples/mock-wallet` is an independent Rust package using core and testkit through path dependencies, as a wallet integration would.

The normalized snapshot is the boundary between wallet-specific proposal construction and deterministic policy rules. This keeps the engine reusable and makes the same finding IDs available to tests, local CLI output, and CI reporting.

## What ShieldSpec does—and does not do

ShieldSpec reports confirmed structural facts in the supplied proposal, such as transparent funding, recipients, or change; unknown routes; and transparent/shielded boundary transitions. Comparison identifies newly introduced or worsened policy findings using deterministic structural fingerprints.

It does not sign or broadcast transactions, access keys or wallet databases, inspect network metadata, deanonymize users, or calculate an anonymity score. A passing assessment means only that no configured rule found a failure in the supplied facts. It is not a guarantee of privacy or anonymity.

ShieldSpec builds on established Zcash privacy concepts and tooling patterns. Its contribution is proposal-aware regression testing and developer ergonomics—not a claim to have invented privacy analysis or to be the first privacy tool. See [the privacy model](docs/privacy-model.md) for the evidence boundary and [the demo storyboard](docs/demo.md) for the judge flow.

## Development

Run the repository checks from the root:

```bash
cargo fmt --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

Run only the independent consumer package with:

```bash
cargo fmt --manifest-path examples/mock-wallet/Cargo.toml --check
cargo test --manifest-path examples/mock-wallet/Cargo.toml
cargo clippy --manifest-path examples/mock-wallet/Cargo.toml --all-targets -- -D warnings
```

CLI exit codes are `0` for a pass/no regression, `2` for a privacy failure/regression, and `1` for an operational or input error.
