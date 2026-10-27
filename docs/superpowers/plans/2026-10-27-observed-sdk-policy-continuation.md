# Observed SDK Policy and Generic Fuzzer Continuation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement task-by-task with red/green tests.

**Goal:** Make ShieldSpec exercise an actual Zcash SDK spend policy and improve truthful differential discovery, independently of any historical privacy-policy bypass reproduction.

**Architecture:** Introduce opt-in wallet spend-policy observations in the existing real proposal runner; keep legacy snapshot-only response unchanged. An independent generic campaign compares observed verdicts and reports coverage; Ironwood cross-pool coverage is extended only to the extent supported by the real linked proposal machinery. Each finding must retain its provenance and replay identity.

**Tech Stack:** Rust, pinned librustzcash backend, serde runner JSON, existing shell-runner CLI integration suite.

**Spec:** `docs/superpowers/specs/2026-10-27-shieldspec-product-sprint-design.md` as amended by the user's explicit exclusion of the historical FullPrivacy bypass test.

## Global Constraints
- ALL repository edits, builds and tests through Remote Desktop Commander on ayobami's Ubuntu laptop, never the internal environment.
- No historical FullPrivacy bypass reproduction, injection, dynamic exploit testing, live wallet, signing, broadcasting, or funds.
- No faked wallet acceptance result or relabeling a snapshot warning as a wallet policy verdict.
- Preserve old scenario JSON, status:ok runner responses, existing CLI and historical evidence.
- Red/green tests, strict typed error distinction, determinism and honest README claims.
- Separate implementation worktrees/branches and file ownership, reviewed before integration.

## Review Focus
1. Non-wallet/proposal failures remain errors, not "rejected".
2. Orchard-only SDK selection accepts adequately funded Orchard inputs; rejects adequately funded Sapling-only inputs.
3. Both decision paths still provide honest diagnostic snapshots and stable policy names.
4. Default runner response remains identical for legacy callers.
5. Wallet-policy difference is preserved after minimization and replay; missing observations do not count as clean policy coverage.

---

### Task 1: Real SDK wallet spend-policy observation

**Files:** Modify `crates/shieldspec-zcash-runner/src/lib.rs`, `crates/shieldspec-zcash-runner/src/bin/shieldspec-zcash-runner.rs`; add focused integration tests in `crates/shieldspec-zcash-runner/tests/` and docs.

**Interfaces:** Opt-in named `zcash_spend_orchard_only_v1` policy uses the actual `zcash_client_backend` `SpendPolicy::shielded_pools([Orchard])` and wallet input-selection proposal path. Serialize real `RunnerResponse::Observed`, with a standard full diagnostic snapshot, for accepted or insufficient-allowed-funds rejection. Other failures are operational errors.

- [ ] Write real-wallet tests for funded Orchard accepted and funded Sapling rejected, opt-in default compatibility, and operational error separation.
- [ ] Verify tests fail for missing implementation.
- [ ] Add real backend policy invocation and CLI mode, no historical FullPrivacy dependency.
- [ ] Re-run Rust targeted tests, formatter/clippy; commit isolated worktree.

### Task 2: Generic campaign coverage and replay hardening

**Files:** Modify `crates/shieldspec-zcash-runner/src/bin/shieldspec-fuzz.rs`, `crates/shieldspec-zcash-runner/tests/differential_cli.rs`.

**Interfaces:** Clean campaign JSON reports cases for which both SDK verdicts were genuinely observed versus unavailable on one or both sides. Final minimized case must be observed to preserve exactly the original discovery before save/output. Replay status must not misstate the original stored finding.

- [ ] Red tests for observed vs unobserved coverage and an alternate replay finding.
- [ ] Implement coverage counters and precise replay status.
- [ ] Guard final minimized re-evaluation against nondeterminism or lost discovery.
- [ ] Verify targeted CLI tests; commit.

### Task 3: Ironwood scenario feasibility and extension

**Files:** Depending on SDK support, `crates/shieldspec-zcash-runner/src/lib.rs` and `tests/scenario_generation.rs` for executable cases; otherwise add a documented capability boundary and interoperable schema (not a pretend execution).

- [ ] Read linked SDK's real Ironwood proposal support and current scenario constraints.
- [ ] Add safe same-pool/cross-pool cases only when backed by actual proposal execution.
- [ ] Add deterministic generator/replay/shrink tests and real SDK smoke test.
- [ ] Verify targeted tests, explicitly record cases blocked by SDK support and keep unsupported inputs as errors.

### Task 4: Integration and verification

**Files:** README, `docs/wallet-policy-observations.md`, other documentation affected by the implementation.

- [ ] Review Task 1/2/3 changes and merge branches without overwriting unrelated work.
- [ ] Run `cargo fmt --all --check`, `cargo test --locked --workspace --all-targets`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, and applicable docs/CLI tests on laptop.
- [ ] Audit that no historical bypass testing was added, no synthetic evidence is described as actual SDK divergence.
- [ ] Commit and update PR against main; report outstanding limitations and links.
