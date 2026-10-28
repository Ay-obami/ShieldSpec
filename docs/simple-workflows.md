# Simple ShieldSpec workflows

The published `shieldspec-cli 0.4.0` provides a short, configuration-driven entry point. Install using Rust 1.95+:

```sh
cargo install shieldspec-cli --version 0.4.0 --locked
```

The first check below needs no source checkout; the example files and differential runners in later sections use a clone of this repository.

## 1. Try a built-in snapshot check

```sh
shieldspec try
```

This runs a bundled, known-good **snapshot fixture** without wallet credentials or a funded account. It does **not** exercise a wallet, run differential fuzzing, or prove anonymity.

## 2. Check your own snapshots with one command

For an immediately runnable, known-good snapshot comparison:

```sh
shieldspec test --config examples/shieldspec-snapshot-control.toml
```

This compares the same committed fixture to itself. For your own snapshots, create `shieldspec.toml` beside your fixture files:

```toml
[comparison]
mode = "snapshot"
baseline = "baseline.json"
candidate = "candidate.json"

[testing]
policy = "standard"
```

Run:

```sh
shieldspec test
```

A clean result exits `0`, an input/operational error `1`, and a configured privacy failure `2`. Relative paths resolve from the configuration file's directory. To choose another file, use `test --config path/to/shieldspec.toml`.

## 3. Compare compatible wallet runners

Build the source tools:

```sh
cargo build --locked -p shieldspec-cli -p shieldspec-zcash-runner --bins
```

Use this sample `shieldspec.toml` from the repository root to exercise a **first-party SDK self-comparison negative control**:

```toml
[comparison]
mode = "differential"
fuzzer_binary = "target/debug/shieldspec-fuzz"
baseline_runner = "target/debug/shieldspec-zcash-runner"
candidate_runner = "target/debug/shieldspec-zcash-runner"
baseline_args = ["--wallet-spend-policy", "orchard-only"]
candidate_args = ["--wallet-spend-policy", "orchard-only"]

[testing]
scenario_family = "ironwood"
policy = "standard"
seed = 2026
cases = 4
```

Run:

```sh
shieldspec test
```

Expect a `no_regression_found` control with four comparable real-wallet policy observations on each side. **This does not test your wallet or establish cross-version agreement**. For meaningful regression testing, replace both runner executables with independently built, protocol-compatible versions of your wallet and retain each executable's provenance. Use the [Rust runner-adapter starter](../examples/runner-adapter/README.md) and [runner contract](runner-protocol-contract.md); ShieldSpec does not automatically generate arbitrary wallet adapters.

Advanced, independently reproducible controls remain available in [the full CLI guide](getting-started.md), [CI guide](github-action.md), and [historical Zallet reproduction](zallet-auto-discovery.md). The earlier `check`, `diff`, `fixture`, `shieldspec-fuzz fuzz`, and `replay` commands remain available. The GitHub Action is separately scoped from the CLI and retains its existing behavior in `v0.4.0`. Level 3 automatic wallet-runner creation remains deferred.
