# Simple ShieldSpec workflows

> **v0.5.0 (unreleased)**: The `init`, `test --compare`, and short `replay` workflows below are under development on `feat/developer-experience-v050`. The published CLI is still **0.4.0**, which supports `try` and configuration-driven `test` but not the new commands. Build the feature branch's CLI to try the v0.5 interface; do not assume `cargo install shieldspec-cli` currently supplies it.

## 1. Install and initialize

To try v0.5 from source (Rust 1.95+):

```sh
git clone https://github.com/Ay-obami/ShieldSpec.git
cd ShieldSpec
git switch feat/developer-experience-v050
cargo build --locked -p shieldspec-cli
```

Use `target/debug/shieldspec` in place of `shieldspec` until the new CLI is published or installed from source. Then run:

```sh
shieldspec init
shieldspec test
```

`init` creates `shieldspec.toml`, `baseline.json`, and `candidate.json` without overwriting existing files. The generated snapshots are identical, bundled **negative controls**. They demonstrate configuration and the reporting pipeline: **they do not exercise your wallet or establish a real privacy guarantee**. Replace these files with actual normalized proposals to check your own snapshots.

The existing `shieldspec try` command remains available as a one-command bundled snapshot check.

## 2. Compare wallet builds

Real differential testing requires compatible wallet-runner executables built separately. ShieldSpec does not generate arbitrary wallet adapters (Level 3 remains deferred). Build the first-party SDK example runner and fuzzer from the same repository:

```sh
cargo build --locked -p shieldspec-zcash-runner --bins
```

For a quick **SDK self-comparison negative control**, use the sample configuration at [`examples/shieldspec-self-control.toml`](../examples/shieldspec-self-control.toml) as a reference, or set your `shieldspec.toml` like this:

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

Then run:

```sh
shieldspec test --compare
```

For this four-case self-control, expect `no_regression_found` with four comparable SDK policy observations. Identical runner binaries **do not** test independent wallet revisions. To investigate a real regression, replace the two runner paths with separately built, compatible baseline and candidate wallet revisions and retain their provenance. See the [runner-adapter starter](../examples/runner-adapter/README.md) and [runner contract](runner-protocol-contract.md).

## 3. Reproduce a saved difference

When a differential run discovers, shrinks, and saves a stable discrepancy, replay the discovery:

```sh
shieldspec replay discovery.json
```

The underlying `shieldspec-fuzz` executable must be available, either through your configured `fuzzer_binary` or separately. Verified replay checks recorded executable hashes and arguments; it refuses silently changed runners. `replay` does not create a discovery by itself.

## 4. Status and advanced controls

| Exit | Meaning |
| --- | --- |
| `0` | No configured failing finding or comparable differential discrepancy |
| `1` | Input, runner, configuration, or protocol error — **not** a privacy verdict |
| `2` | Configured privacy finding or changed comparable wallet-policy decision; investigate before calling it a leak |

Paths in `shieldspec.toml` resolve relative to the configuration file. `shieldspec test --config other.toml` remains supported; the existing `check`, `diff`, `fixture`, and direct `shieldspec-fuzz` commands remain supported for automation and precise investigations.

Long-form, explicitly pinned commands remain documented in the [advanced getting-started guide](getting-started.md), [wallet-policy observation reference](wallet-policy-observations.md), [CI guide](github-action.md), and [historical Zallet reproduction](zallet-auto-discovery.md). Keep these exact arguments when reproducing published evidence; a shorter command is not automatically an identical experiment.

The separately published GitHub Action retains its documented behavior. **Level 3 automatic wallet-runner creation remains deferred.**
