# Rust wallet runner adapter starter

This is a **synthetic integration starter**, not a wallet implementation or a privacy audit. It builds independently of ShieldSpec's root workspace and uses the existing v1 runner protocol.

## Try it

From the ShieldSpec repository root:

```bash
cargo +1.95.0 build --locked --manifest-path examples/runner-adapter/Cargo.toml
cargo +1.95.0 run --locked -p shieldspec-fuzz --bin shieldspec-validate-runner -- \
  examples/runner-adapter/target/debug/runner-adapter \
  examples/runner-adapter/fixtures/scenario.json --demo
```

The preflight checks one scenario and the runner's JSON envelope and exit code. It does **not** establish wallet coverage or privacy.

Without `--demo`, the starter intentionally returns an operational error. With `--demo`, it emits a synthetic, valid **fixed shielded-payment snapshot** regardless of the scenario; this is useful only for protocol wiring checks and never represents observed wallet behavior.

## Connect a real wallet

1. Replace `ExampleWallet::execute` in [src/main.rs](src/main.rs) with your wallet's actual scenario-to-proposal and proposal-to-snapshot adapter. Keep JSON off stdout except for the single response.
2. Interpret the [scenario envelope](../../crates/shieldspec-zcash-runner/src/lib.rs) for each family you support. Unsupported families must produce `status: "error"` and exit nonzero, not a fake successful snapshot.
3. Return `RunnerResponse::Ok` with a **complete observed proposal snapshot** or `RunnerResponse::Observed` only after invoking an actual **named** wallet policy. Legitimate policy rejection is an observed success; process/SDK failure is an operational error.
4. Compile two independent wallet revisions and run the preflight separately against each executable. Only then compare with `preset: custom` and preserve build SHAs, args and artifacts.

See [runner protocol](../../docs/runner-protocol-contract.md). A [workflow recipe](.github/workflows/differential.yml) shows the mechanics using the **same demo binary twice as a negative control**. Its paths assume a ShieldSpec source checkout; adapt the checkout, manifest, binary paths, and validator build when integrating an external wallet repository. Replace both sides with real runner builds for meaningful comparisons. This workflow is not automatically enabled by the repository. Do not treat a passing demo as proof of wallet privacy.
