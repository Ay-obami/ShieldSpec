# Runner Adapter Starter Design

## Intent
Reduce integration friction for wallet teams trying the v0.3.0 differential Action. Provide a copyable Rust runner template, a protocol preflight validator, and an opt-in GitHub Actions example.

## Requirements and boundaries
- Keep the existing v1 JSON runner contract and backward-compatible legacy support. A runner reads exactly one scenario from stdin and emits one valid JSON response to stdout.
- Treat policy rejection as an observed, successful invocation; return operational errors separately. Do not invent wallet-policy decisions when instrumentation is absent.
- Template must run unchanged as a clearly labeled example and make wallet-specific integration points explicit; it must not claim to run an arbitrary wallet.
- Preflight validates structural protocol behavior and exit codes without falsely claiming real wallet coverage.
- CI example builds baseline/candidate executables and runs the v0.3.0 Action with artifacts.
- Avoid modifying the core fuzzer protocol or adding unrelated dependencies; retain Rust 1.95 compatibility.

## Approaches considered
1. Documentation-only copy/paste snippets: lowest maintenance, but no runnable validation.
2. **Runnable Rust starter + validator + workflow (chosen):** a modest maintained example with executable smoke tests and a clear adoption path.
3. Automatic wallet discovery/code generation: less manual work eventually, but SDK-specific complexity and correctness risk; reserved for Level 3.

## Design
Add `examples/runner-adapter/` as an independent Rust workspace example with a small `WalletAdapter` trait and a demonstrative `ExampleWallet` implementation mapping a normalized scenario into an illustrative, complete snapshot. Add a `shieldspec-fuzz validate-runner` subcommand, or a narrowly scoped validation script if CLI wiring is disproportionately invasive, which sends a fixture over stdin and checks the response with the existing `parse_runner_response` contract. Add tests for malformed JSON, successful observed/ok responses, error exit behavior, and incorrect policy evidence. Add an opt-in `.github` workflow example under `examples/runner-adapter/` rather than enabling a costly default CI job. Document caller obligations and protocol limits.

## Exclusions
No automatic third-party repository cloning, no credentials/signing/broadcast, no assumed wallet SDK compatibility, no new crates.io publication, no change to Action semantics.
