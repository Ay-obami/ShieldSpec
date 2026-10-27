# ShieldSpec One-Sprint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a reusable Zcash proposal privacy-regression testkit with CLI, SARIF, GitHub Action, test fixtures, and an independent consumer example.

**Architecture:** A small Rust workspace normalizes transaction-proposal facts in `shieldspec-core`; `shieldspec-zcash` converts real Zcash SDK proposals; `shieldspec-testkit` builds deterministic fixtures; `shieldspec-cli` exposes checks and regression diffs. GitHub Actions wraps the CLI without wallet secrets.

**Tech Stack:** Rust stable, serde/serde_json, clap, thiserror/anyhow, zcash_client_backend family matching LeakLens NU7 integration, proptest, GitHub Actions, SARIF 2.1.0.

**Spec:** `docs/superpowers/specs/2026-10-27-shieldspec-design.md`

## Global Constraints

- Testnet/developer tooling only; no signing, broadcasting, keys, seed phrases, or wallet DBs.
- Deterministic explainable rules; no anonymity score.
- Exit codes: 0 pass, 2 privacy failure/regression, 1 operational/input error.
- Human, JSON, and SARIF outputs must describe the same findings.
- All production behavior must follow a red → green → refactor TDD cycle.
- Public API names and rule IDs are fixed by the design spec.
- Parallel workers may not edit another issue's owned files.

## Review Focus

- Multi-step proposals where only a later step leaks.
- Unknown pools/routes must fail closed rather than silently pass.
- Fingerprints must stay stable across message/value-only changes.
- Candidate regressions must not be hidden by unrelated baseline findings.
- Malformed snapshot/schema input must return operational error, never a privacy pass.

---

### Task 1 / Issue 1: Workspace + core policy engine

**Files:**
- Create: `Cargo.toml`
- Create: `crates/shieldspec-core/Cargo.toml`
- Create: `crates/shieldspec-core/src/lib.rs`
- Create focused modules under `crates/shieldspec-core/src/`
- Test: core unit tests

**Interfaces:** Produces every locked core type plus `assess` and `compare`.

- [ ] Write failing tests for each rule ID, standard/strict policy, multi-step leak, stable fingerprints, and regression semantics.
- [ ] Run core tests and record expected RED failures.
- [ ] Implement minimal model + policy engine.
- [ ] Run `cargo test -p shieldspec-core`.
- [ ] Run `cargo fmt --check` and `cargo clippy -p shieldspec-core --all-targets -- -D warnings`.
- [ ] Commit with `feat(core): add proposal privacy policy engine`.

### Task 2 / Issue 2: Zcash proposal adapter

**Files:**
- Create: `crates/shieldspec-zcash/Cargo.toml`
- Create: `crates/shieldspec-zcash/src/lib.rs`
- Test: adapter tests using real SDK proposal fixture construction

**Interfaces:** Consumes core snapshots; produces `snapshot_from_proposal(...)->Result<ProposalSnapshot, AdapterError>`.

- [ ] Pin compatible Zcash SDK dependencies based on LeakLens's verified NU7 family.
- [ ] Write failing tests proving transparent input, selected transparent receiver/change, fee, and late-step facts survive conversion.
- [ ] Verify RED.
- [ ] Implement adapter without signing/broadcast capabilities.
- [ ] Run package tests + strict Clippy.
- [ ] Commit `feat(zcash): adapt SDK proposals into privacy snapshots`.

### Task 3 / Issue 3: Testkit + canonical fixtures + property tests

**Files:**
- Create: `crates/shieldspec-testkit/Cargo.toml`
- Create: `crates/shieldspec-testkit/src/lib.rs`
- Create: `fixtures/*.json`
- Test: builder/fixture/property tests

**Interfaces:** Produces `ProposalBuilder`, `StepBuilder`, fixture lookup/list/write helpers.

- [ ] Write failing builder ergonomics and canonical fixture assertions.
- [ ] Add property tests for fingerprint determinism and monotonic addition of transparent exposure.
- [ ] Verify RED.
- [ ] Implement minimal builders and all eight required fixtures.
- [ ] Verify fixture JSON roundtrips.
- [ ] Commit `feat(testkit): add proposal builders and privacy vectors`.

### Task 4 / Issue 4: CLI + human/JSON/SARIF reporters

**Files:**
- Create: `crates/shieldspec-cli/Cargo.toml`
- Create: `crates/shieldspec-cli/src/main.rs`
- Create focused command/reporter modules
- Test: CLI integration/snapshot tests

**Interfaces:** Implements locked `check`, `diff`, and `fixture` commands and exit-code contract.

- [ ] Write failing CLI tests for pass/fail/error exit codes.
- [ ] Write failing reporter tests proving equivalent rule IDs/fingerprints in human, JSON, SARIF.
- [ ] Verify RED.
- [ ] Implement commands/reporters.
- [ ] Run CLI tests and strict Clippy.
- [ ] Commit `feat(cli): add privacy regression CLI and SARIF output`.

### Task 5 / Issue 5: GitHub Action + CI

**Files:**
- Create: `action.yml`
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/shieldspec-demo.yml`
- Test: action smoke workflow and local validation where possible

**Interfaces:** Action inputs `candidate`, optional `baseline`, `policy`, `format`.

- [ ] Add a deliberately regressing fixture command in CI and assert exit 2.
- [ ] Add a passing baseline/candidate check.
- [ ] Wire format/test/clippy plus action smoke jobs.
- [ ] Ensure action requires zero secrets.
- [ ] Commit `ci: add ShieldSpec GitHub Action and regression gate`.

### Task 6 / Issue 6: Independent consumer example + docs/demo

**Files:**
- Create: `examples/mock-wallet/Cargo.toml`
- Create: `examples/mock-wallet/src/main.rs`
- Create: `README.md`
- Create: `docs/demo.md`
- Create: `docs/privacy-model.md`

**Interfaces:** Example consumes `shieldspec-core` and `shieldspec-testkit` as a downstream developer would.

- [ ] Write example test that fails when candidate introduces transparent change.
- [ ] Verify RED before wiring the library.
- [ ] Implement minimal independent consumer.
- [ ] Document five-minute local demo and judge-facing magic moment.
- [ ] Document limitations and prior-art-safe positioning.
- [ ] Commit `docs: add independent wallet integration and demo`.

### Task 7 / Issue 7: Integration, whole-branch review, release readiness

**Files:** Integration only; no new architecture unless a verified blocker requires it.

- [ ] Merge issue branches after package-level checks.
- [ ] Run `cargo fmt --check`.
- [ ] Run `cargo test --workspace --all-targets`.
- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Run CLI demo: shielded baseline vs transparent-change candidate and verify exit 2.
- [ ] Generate and validate SARIF JSON.
- [ ] Build/test the GitHub Action in CI.
- [ ] Run independent code review and fix all Critical/Important findings.
- [ ] Confirm README reproduces the demo from a clean checkout.
- [ ] Tag/release only after fresh verification evidence.
