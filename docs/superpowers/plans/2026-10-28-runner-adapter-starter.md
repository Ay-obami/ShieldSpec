# Runner Adapter Starter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make custom wallet differential CI integration easier with a working Rust runner starter, protocol preflight, and workflow recipe.

**Architecture:** Reuse the current stdin/stdout runner contract and `parse_runner_response` instead of inventing a second wire format. Keep an independent example crate and a small preflight surface in the existing fuzzer CLI or validation script.

**Tech Stack:** Rust 1.95, Cargo, serde_json, GitHub Actions YAML.

**Spec:** `docs/superpowers/specs/2026-10-28-runner-adapter-starter-design.md`

## Global Constraints
- Preserve the v1 and legacy runner protocol.
- Never conflate wallet rejection with process errors.
- The example never claims to test a real wallet or submit transactions.
- No new crates.io releases or behavioral changes to the existing Action.

## Review Focus
- Empty stdout: preflight rejects missing response.
- Non-JSON stdout: preflight returns a protocol failure.
- Observed rejection with exit 0: accepted as valid observation.
- Nonzero exit after successful-looking output: reported as operational error.
- Missing or blank observed policy: rejected as invalid protocol.

---

### Task 1: Runnable starter and contract tests
**Files:** Create `examples/runner-adapter/Cargo.toml`, `examples/runner-adapter/src/main.rs`, `examples/runner-adapter/README.md`, `examples/runner-adapter/tests/runner.rs`.
**Interfaces:** `WalletAdapter::execute(&self, scenario: &serde_json::Value) -> Result<RunnerResponse, String>`; runner reads one JSON scenario from stdin, writes one JSON response to stdout.
- [ ] Write integration tests exercising valid fixture, invalid JSON, and operational errors against runner executable.
- [ ] Run tests to observe expected failures before implementation.
- [ ] Implement the minimal example adapter and runner protocol envelope.
- [ ] Run `cargo test --manifest-path examples/runner-adapter/Cargo.toml` and confirm green.

### Task 2: Protocol preflight
**Files:** Modify `crates/shieldspec-zcash-runner/src/bin/shieldspec-fuzz.rs` or add `scripts/validate-runner.py` and its tests, choosing the smallest reuse of `parse_runner_response`.
**Interfaces:** Validate a supplied runner command using a known fixture and report whether it emits structurally valid protocol responses and consistent exit codes; never claim a wallet was checked.
- [ ] Write tests for the five failure modes in Review Focus, including valid observed rejection.
- [ ] Run preflight tests red before implementation.
- [ ] Implement preflight with stable diagnostic messages.
- [ ] Run targeted tests green and existing differential protocol tests.

### Task 3: CI adoption recipe and full verification
**Files:** Create `examples/runner-adapter/.github/workflows/differential.yml`; update `docs/ci-integration.md`, `README.md` and starter README.
**Interfaces:** Show two runner build steps and `Ay-obami/ShieldSpec@v0.3.0` using `preset: custom`; distinguish template control from real wallet checks.
- [ ] Document exact copy/build/validate/run steps with reusable relative runner paths.
- [ ] Add a smoke check that exercises the documented commands.
- [ ] Run Rust fmt/tests/Clippy, existing workspace tests, and YAML/documentation checks.
- [ ] Review diffs, commit on isolated branch, push and open PR. Merge only after relevant CI succeeds.
