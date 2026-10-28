# TDD record

The downstream regression test was written before the example implementation.

Initial RED command:

```console
$ cargo test --manifest-path examples/mock-wallet/Cargo.toml
error: failed to load manifest for dependency `shieldspec-core`
...
failed to read `crates/shieldspec-core/Cargo.toml`
...
No such file or directory (os error 2)
```

Cargo exited with status 101 at that **initial implementation checkpoint**: the concurrently developed `shieldspec-core` and `shieldspec-testkit` path dependencies had not yet landed in the branch. Those dependencies are now part of the integrated ShieldSpec repository. The historical RED output above is retained as a TDD record, **not** as the current build status.

To run the downstream example from the repository root on current `main`:

```bash
cargo test --locked --manifest-path examples/mock-wallet/Cargo.toml
```

The example depends directly on the real `shieldspec-core` and `shieldspec-testkit` path crates, with no stand-ins. Its passing or failing status should be reported from a fresh run, not inferred from the historical RED log.
