# ShieldSpec Product Sprint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** General differential fuzzing rediscovers real historical Zallet FullPrivacy cross-pool behavior, and ShieldSpec ships an accessible developer-first product landing page and onboarding.

**Architecture:** Five isolated independent workstreams: scenario domain, pinned real Zallet runner adapter, privacy verdict/oracle, static product website, and documentation. An integration task joins compatible runner results with shrink/replay and provenance; final task verifies and packages. Integrate into `feat/historical-sdk-proof` through reviewed branches.

**Tech Stack:** Rust workspace, pinned Zallet/zallet-core (historical), serde JSON runner IPC, Python historical reproducer, HTML/CSS/vanilla JavaScript static website, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-27-shieldspec-product-sprint-design.md`

## Global Constraints
- All ShieldSpec file, test and Git operations MUST run through Remote Desktop Commander on the user's Ubuntu laptop; never use the internal executor for project work.
- Preserve existing `Scenario`, `ProposalSnapshot`, CLI, GitHub Action, evidence, historical scripts and current Rust tests, except backward-compatible extensions.
- Verify real `zallet-core` policy behavior under fixed baseline `e966780216b7a86b0a1a6618f2fde5b574da7171` and affected candidate `f496c9b6b2c6ebb0065b329c11bda4a5fdf73568`.
- Distinguish proven `FullPrivacy` policy breaches from snapshot privacy warnings, ordinary operational errors and expected ZIP-320 ephemeral bridges.
- Report honest historical provenance, not a newly found vulnerability.
- Use fresh failing tests first and capture passing Rust/site checks before claiming completion.
- Independent issue branches/worktrees; avoid overlapping production files. One integration owner resolves shared interfaces.

## Review Focus
1. Ironwood/Sapling cross-pool case has an actual value-balance exposure; no false positive for same-pool Ironwood.
2. Permissive privacy policies and real Zallet result distinctions do not create spurious regressions.
3. Missing/older runner verdict fields remain backwards compatible with snapshot-only JSON.
4. Shrinker never turns a valid failing original into an invalid/non-failing minimized case.
5. Website demo labels historical/test artifacts clearly; keyboard and mobile accessibility remain usable.

---

### Task 1: Valid semantic cross-pool scenarios

**Files:** Modify `crates/shieldspec-zcash-runner/src/lib.rs`; extend `crates/shieldspec-zcash-runner/tests/scenario_generation.rs`.

**Interfaces:** Produce a backward-compatible policy-check scenario variant that describes funding pool, target pool, public value-balance input, privacy policy, and deterministic seed/ordinal. Existing Orchard/TEX proposal cases must retain serialization and behavior.

- [ ] Write a test generating a valid FullPrivacy Ironwood→Sapling policy scenario and a same-pool control; run `cargo test -p shieldspec-zcash-runner --test scenario_generation` and observe the intended failure.
- [ ] Implement bounded generator selection/validation and semantic shrinking support for this scenario, with conservation of values and policy choice.
- [ ] Repeat test and run `cargo test --locked -p shieldspec-zcash-runner`; commit to the issue branch.

### Task 2: Dynamic real Zallet policy runners

**Files:** Extend `evidence/historical-zallet-677/shieldspec_privacy_history.rs`, `scripts/reproduce_zallet_677.py` or a focused new adapter file; add tests in `scripts/tests/test_historical_zallet_reproduction.py`.

**Interfaces:** Runner accepts the Task 1 policy-case JSON over stdin or a deterministic JSON path and produces machine-readable observations: `sdk_revision`, `policy`, `actual_policy_verdict` (`accepted`/`rejected`), actual output-side public balances, and operational errors distinctly. Both pinned upstream revisions execute the same dynamic fixture.

- [ ] Add tests for dynamic Ironwood→Sapling and controls; observe expected failure under old fixed-only witness.
- [ ] Implement shared injected Rust fixture invoked by real upstream `zallet-core`, with strict schema checks and transparent revision provenance.
- [ ] Verify affected accepts vs fixed rejects, two controls agree, and `python3 -m unittest discover -s scripts/tests -v` passes; commit.

### Task 3: Cross-pool oracle and differential policy verdict

**Files:** Modify `crates/shieldspec-core/src/rules.rs`, `crates/shieldspec-fuzz/src/lib.rs`, associated `crates/shieldspec-core/tests/policy_engine.rs` and `crates/shieldspec-fuzz/tests/differential.rs`.

**Interfaces:** Extend runner message protocol backward-compatibly with optional actual wallet policy observation. Add an explicit policy breach classification for `FullPrivacy` cross-pool public value balance. Compare real policy accept/reject against explicit expectation and across baseline/candidate. Absence of a policy verdict is not itself regression.

- [ ] Write red tests: full-privacy cross-pool accepted is privacy breach; same-pool allowed; permissive policy allowed; old runner responses decode.
- [ ] Implement first-class policy observation and oracle independent of mere pool-label changes; preserve existing snapshot rules and result formats.
- [ ] Run core/fuzz test suites and relevant clippy; commit.

### Task 4: Developer-first product website

**Files:** Create `site/index.html`, `site/styles.css`, `site/app.js`, `site/data/*.json`, `site/README.md`, and a lightweight site validation test/script.

**Interfaces:** Standalone static site, relative asset links, verified human-readable sample reports from checked-in evidence, no runtime backend. Product CTAs go to GitHub and actual docs/quickstart.

- [ ] Write red static checks for title, semantic landmarks, responsive viewport, accessibility labels, real evidence links and explicit “historical reproduction” wording; verify failure on missing site.
- [ ] Build polished responsive premium UI with hero, problem, real evidence, steps, interactive report browser, integrations, install commands, limitations, roadmap/FAQ; no invented customers/adoption stats.
- [ ] Verify static tests and local HTTP rendering on laptop via Remote Commander (browser if available); commit.

### Task 5: Developer onboarding and release identity

**Files:** Update `README.md`, `docs/privacy-model.md`, `docs/submission.md`, create `docs/getting-started.md`, `docs/roadmap.md`; coordinate links with website agent (docs are exclusive to this task).

**Interfaces:** Exact CLI commands verified against code; clearly distinguish privacy-balance oracle from snapshot findings and show independent controls, pinned-version provenance, honest scope. Website links stable.

- [ ] Write/fix documentation contract checks for runnable quickstarts, source links, controls, reproducibility and caveats; fail intentionally on missing pieces.
- [ ] Author product-focused installation, CI integration, scenario replay, limitations, contribution/release roadmap; update submission without claiming new upstream bug.
- [ ] Execute documented commands on remote laptop; commit.

### Task 6: General fuzzer integration, shrinking, replay, CI

**Files:** Modify `crates/shieldspec-zcash-runner/src/bin/shieldspec-fuzz.rs`, `crates/shieldspec-zcash-runner/tests/discovery_provenance.rs`, `crates/shieldspec-zcash-runner/tests/process_protocol.rs`, and a dedicated manual historical GitHub workflow.

**Interfaces:** Use compatible Task 1 scenario, Task 2 genuine Zallet policy adapters, and Task 3 verdict/oracle. Persist seed, ordinal, original/minimized valid JSON, hashes and exact revisions. Corrected version is baseline, affected version candidate, consistent naming.

- [ ] Write red end-to-end test that the ordinary general fuzz campaign finds documented historical difference without `--demo-regression`; controls yield no findings.
- [ ] Implement runner dispatch and semantic minimization/replay for policy cases; make failures distinct from diagnostic errors.
- [ ] Run deterministic campaign twice, compare finding identities, run full Rust test suite and optional historical job; commit.

### Task 7: Integration, verification, product launch

**Files:** Integration-only merge conflict resolution, `site/` polish, release docs, `docs/media/` verified demo update if generation is available.

**Interfaces:** No rewriting reported evidence. Make final repo ready for actual developers, clearly distinguish a website published URL from laptop preview.

- [ ] Review each workstream's commits and tests, integrate into one feature branch or PR; preserve previous branch state.
- [ ] Verify `cargo fmt --all --check`, `cargo test --locked --workspace --all-targets`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, Python test suite, static site tests, historical old/new reproduction, and provenance replay.
- [ ] QA landing on desktop/mobile via available remote browser, ensure broken links and readability checks pass, produce accurate demo/readme and final evidence manifest.
- [ ] Commit integration and push authorized branch/PR; return issue URLs, PR, test evidence and actual deployment/preview status.

## Parallel Execution Contract
Tasks 1–5 may run in parallel only in isolated worktrees; Task 6 starts after interface contracts are confirmed; Task 7 integrates at the end. Agents must never modify another task's files or use local/internal code execution. Separate implementation and review agents where feasible. No unverified success claims.
