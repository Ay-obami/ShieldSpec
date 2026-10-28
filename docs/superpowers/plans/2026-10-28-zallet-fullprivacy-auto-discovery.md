# Historical Zallet FullPrivacy Auto-Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make ShieldSpec's general fuzzer automatically discover the documented historical FullPrivacy decision difference between two independently compiled real Zallet policy implementations, with faithful generated inputs and deterministic replay.

**Architecture:** Extend the existing scenario generator with a dedicated balanced one-input/one-change Zallet policy probe family. Build an identical test-only dynamic Rust policy-probe module inside the two pinned Zallet revisions, then use a hash-checked Python transport wrapper to translate actual emitted Proposal observations to the existing v1 process protocol. Reuse the current differential fuzzer, minimizer and replay, not a separate verdict-comparison script.

**Tech Stack:** Rust 1.95, Cargo, Python 3 stdlib, serde_json, Git worktrees, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-28-zallet-fullprivacy-auto-discovery-design.md`

## Global Constraints
- Historical fixed reference `e966780216b7a86b0a1a6618f2fde5b574da7171`; affected candidate `f496c9b6b2c6ebb0065b329c11bda4a5fdf73568`. Do not reverse labels silently.
- Identical injected test-only Rust source; no modifications to Zallet production policy logic. Fixed original historical witness and result hashes remain unchanged.
- Every observed decision must come from real `enforce_privacy_policy` on a successfully constructed genuine `zcash_client_backend::Proposal`. Complete normalized snapshot facts must come from the typed Proposal, not be fabricated from input JSON.
- Dedicated schema `{"kind":"zallet_privacy_probe","params":{"input_pool":"ironwood","change_pool":"sapling","input_zat":20000,"fee_zat":10000}}`; change value is input minus fee. A one-change/fee Proposal with empty TransactionRequest is not a payment to a recipient.
- Both accepted and rejected policy observations are successful protocol v1 `status:"observed"` responses under policy `zallet_full_privacy_v1`; errors/missing output must fail closed with operational exit 1.
- No user keys, mainnet broadcasts, purported newly found upstream zero-days, unverified snapshot/proposal results, or hardcoded historical verdict table.
- Every persisted discovery records seed, original/minimized scenario, versions, adapter and compiled-runner SHA-256; replay checks hashes. Keep current demo and main untouched until verified.

## Review Focus
- Empty/invalid case input or unknown pools must fail before invoking Zallet, never be treated as a rejection (Task 2 test).
- Policy Err must emit `rejected` with exit 0, not an operational crash (Task 2 test).
- Missing, duplicate or malformed `SHIELDSPEC_POLICY_PROBE_JSON` markers fail closed (Task 2 test).
- Hash mismatch or wrong pinned revision fails closed before running/replaying (Task 2 test).
- Saved minimized differential must reproduce the same named wallet policy difference with both immutable executables (Task 3 test).

---

### Task 1: Dedicated Zallet privacy probe generator

**Files:**
- Modify `crates/shieldspec-zcash-runner/src/lib.rs`.
- Modify `crates/shieldspec-zcash-runner/src/bin/shieldspec-fuzz.rs`.
- Create `crates/shieldspec-zcash-runner/tests/zallet_privacy_scenarios.rs`.

**Interfaces:**
- `ZalletPrivacyProbeScenario { input_pool: ZalletInputPool, change_pool: ZalletChangePool, input_zat: u64, fee_zat: u64 }`; define only `orchard|ironwood` inputs and `sapling|orchard|ironwood` changes, serde snake_case.
- New `Scenario::ZalletPrivacyProbe` serializes with `kind:"zallet_privacy_probe"`.
- `ScenarioGenerator::zallet_privacy_case_at(seed:u64, ordinal:u64)->GeneratedCase` emits deterministic balanced values and cycles 3 controls: Ironwood→Ironwood, Orchard→Sapling, Ironwood→Sapling.
- `shieldspec-fuzz fuzz --scenario-family zallet-privacy` and `replay --scenario-family zallet-privacy` route to this generator; minimization only outputs valid cases with fee<input. For this family require comparable observed wallet-policy decisions on each attempt, otherwise fail operationally.

- [ ] Write Rust tests for stable serde shape, seed/ordinal determinism, pair coverage, balanced shrink and missing-policy fail-closed campaign.
- [ ] Run tests RED and record failures.
- [ ] Implement minimal types/generator/CLI/shrink and strict observation check; preserve other families.
- [ ] Run focused tests GREEN, `cargo fmt --check`, and existing scenario/protocol regressions.
- [ ] Commit only Task 1 files.

### Task 2: Real pinned Zallet policy runners and strict adapter

**Files:** New `evidence/historical-zallet-677/shieldspec_dynamic_policy_probe.rs`, `scripts/build_zallet_policy_runners.py`, `scripts/zallet_policy_runner.py` and Python unit tests. Preserve the old fixed witness, script and committed evidence.

- [ ] RED tests for malformed scenario, invalid balance, missing/duplicate marker, mismatched observed proposal facts and mismatched binary hash.
- [ ] Implement source-only `#[cfg(test)]` injection in each pinned upstream revision; compile two independent zallet-core test binaries with shared low-disk Cargo target; copy each executable to its immutable side before next compile.
- [ ] Produce provenance manifest with precise source commits, fixture hash, binary SHA-256 and SDK versions; validate inputs against that manifest before invocation.
- [ ] In Rust build actual Proposal with chosen Ironwood/Orchard input, Sapling/Orchard/Ironwood change, positive fee; inspect typed Proposal APIs and call actual `enforce_privacy_policy(FullPrivacy)`.
- [ ] Python stdin/stdout v1 adapter runs real compiled test binary and verifies exactly one truthful policy marker; JSON snapshots derived from the typed proposal. Rejection is observed result, operational failure exits nonzero.
- [ ] Commit Task 2 in separate worktree.

### Task 3: CI, live campaign, replay, and truthful documentation

**Files:** `.github/workflows/zallet-auto-discovery.yml`, new validation script, README, docs/guides/roadmap/evidence only after real verification.

- [ ] Merge both independent feature branches into this integration worktree after targeted tests.
- [ ] Build pinned policy binaries, run deterministic controls and real 3-case general campaign, confirm exit 2 named `wallet_policy_difference` and save SHA-pinned discovery.
- [ ] Replay saved minimized discovery against identical executables and confirm same policy verdict direction. Tamper a binary hash and require fail-closed operational exit 1.
- [ ] CI independent clean checkout builds and validates both Zallet binaries, controls, discovery and saved replay, uploads manifest, logs and case artifacts; runner disk use bounded.
- [ ] Update docs to distinguish real upstream historical rediscovery from manufactured demo fault and from current/new vulnerability.
- [ ] Run Rust, Python, formatting and CI checks; seek code review, commit and open PR only after evidence holds. Keep original captioned demo intact.
