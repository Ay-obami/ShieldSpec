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

Cargo exited with status 101, as expected: the concurrently developed
`shieldspec-core` and `shieldspec-testkit` path dependencies had not landed in
this branch. Once those crates are integrated, this package's test is the GREEN
check; no local stand-ins for either crate are included here.
