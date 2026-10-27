# Historical Zcash SDK Evidence and Submission Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Reproduce one genuine upstream Zcash SDK behavioral change, preserve verifiable runner/replay provenance, and ship honest Zecathon demo/submission materials.

**Architecture:** A sandboxed Python historical harness checks out two exact librustzcash commits and executes the *same* upstream regression test against each. The existing Rust fuzzer records binary hashes plus declared SDK provenance in discoveries and enforces them during replay. A clear release dossier documents actual observations and a screen-recording script, without conflating correctness, privacy, and demo injection.

**Tech Stack:** Git, Rust 1.95, Cargo, Python 3 standard library, JSON, GitHub Actions.

**Spec:** User's three-point release-polish request, plus `docs/superpowers/specs/2026-10-27-shieldspec-design.md`.

## Global Constraints

- Must use the actual SDK proposal machinery; no manufactured SDK changes.
- Distinguish documented send-max correctness difference from privacy findings.
- Pin exact pre/post commits `54ff673169ec8be5a4b023a7ae69bcf5caead2a3` and `25766a4c754a847b0a9e3a63801d8f46a270c01b`.
- Keep any demo-only injected privacy fault explicitly labeled.
- No wallet secrets, mainnet spending, or broadcasting.
- Record seed, ordinal, original and minimized scenario, runner hashes and SDK labels.

## Review Focus

- An invalid SDK checkout must abort, not produce an evidence report.
- An upstream test absent on the old commit must be backported without including its fix.
- A compilation failure must never be reported as a historical SDK behavioral failure.
- A replay with changed runner binary must be rejected, unless the user explicitly overrides it.
- An expected ZIP-320 transparent bridge must never be labeled a novel privacy leak.

---

### Task 1: Historical reproducibility harness

**Files:** `scripts/reproduce_librustzcash_2614.py`, `scripts/tests/test_historical_reproduction.py`, `evidence/historical-2614/README.md`, `.github/workflows/historical-sdk-reproduction.yml`.

**Interfaces:** command `python3 scripts/reproduce_librustzcash_2614.py --output <dir> [--before-checkout <dir> --after-checkout <dir>]`; JSON verdict distinguishing an expected pre-fix failed assertion from corrected pass; source hashes, commit hashes, and complete test logs.

- [ ] Write unit tests for source extraction/patching, SHA verification and result classification; run them RED.
- [ ] Implement pinned git checkout and identical source test injection into older SDK without applying production fixes.
- [ ] Run the before and after regression tests and verify the *specific* `PaymentPoolsMismatch` failure then success; classify any other error as inconclusive.
- [ ] Write unambiguous CI workflow and historical evidence instructions; run unit tests GREEN.
- [ ] Commit this milestone.

### Task 2: Binary-identifiable replay corpus

**Files:** `crates/shieldspec-fuzz/src/lib.rs`, `crates/shieldspec-zcash-runner/src/lib.rs`, `crates/shieldspec-zcash-runner/src/bin/shieldspec-fuzz.rs`, Rust tests, `Cargo.toml` and `Cargo.lock`.

**Interfaces:** optional `Discovery.provenance` with baseline/candidate executable path, SHA-256 digest, arguments and explicitly labeled SDK revision. `replay --input` restores saved runner pairs and validates hashes or fails closed.

- [ ] Write Rust unit/integration tests for deterministic identities and replay mismatch; run RED.
- [ ] Implement checksum/provenance recording and replay; preserve compatibility with existing corpus JSON.
- [ ] Run `cargo fmt --check`, selected tests and Clippy; run GREEN.
- [ ] Commit this milestone.

### Task 3: Honest submission package

**Files:** `README.md`, `docs/demo.md`, `docs/submission.md`, `docs/demo-video-script.md`, plus optional captured terminal artifact.

**Interfaces:** a 2–3 minute demo flow with one authentic upstream before/after correctness check, separate privacy regression injection, deterministic replay and ZIP-320 non-false-positive, evidence links and CI job.

- [ ] Document evidence and terminology; avoid claims of an upstream privacy discovery.
- [ ] Add reproducible demo script, shot-by-shot narration and submission description.
- [ ] Verify commands and links; run CI-level tests.
- [ ] Commit and prepare PR, but do not merge or publish a hackathon submission without user approval.
