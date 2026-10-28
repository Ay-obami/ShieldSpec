# Differential Action implementation plan

> For agentic workers: use the executing-plans and test-driven-development skills task by task.

**Goal:** Add backwards-compatible, opt-in differential GitHub Action with built-in Zcash SDK preset and custom runner mode.

**Architecture:** Composite Action delegates input validation and fuzzer invocation to a shell script; runner paths are resolved in caller workspace, and report/discovery artifacts are uploaded before the final status is enforced. The built-in preset uses a negative-control self-comparison.

**Tech stack:** Bash, Rust 1.95.0, Python unittest, GitHub Actions YAML.

**Spec:** docs/superpowers/specs/2026-10-28-differential-action-design.md

## Global constraints
- Do not move v0.2.0 or publish v0.3.0 before consumer workflow verification.
- Preserve snapshot semantics and exit 0/1/2.
- No eval or shell interpolation of arbitrary user strings; validate numeric and enum inputs.
- No arbitrary wallet compilation; build first-party fuzzer/runner only.
- Keep public discovery/report evidence, without new privacy-vulnerability claims.

## Review focus
- Empty snapshot candidate fails with status 1, not a silent differential run.
- Custom nonexistent or non-executable runner fails before campaign.
- Built-in preset rejects runner overrides, and states it is a negative control.
- Malformed cases/seed/scenario family/args cannot produce shell injection.
- Artifact upload on exit 2 cannot hide the actual failing CI status.

### Task 1: Test-first entrypoint
**Files:** Create scripts/action-entrypoint.sh, scripts/tests/action_entrypoint_test.py.
**Interface:** Environment-configured mode, preset, snapshot paths, runner paths, args, seed/cases/policy/format; GITHUB_OUTPUT status and report paths.
- [ ] Write unittest cases for validation, path resolution, argument boundaries and exit 0/1/2 with an executable test fuzzer; watch them fail against absent entrypoint.
- [ ] Implement minimal safe bash dispatch and evidence paths, with no eval.
- [ ] Rerun unittest to pass, and run all scripts/tests.
- [ ] Commit tested entrypoint and tests.

### Task 2: Composite Action integration
**Files:** Modify action.yml, .github/workflows/ci.yml.
**Interface:** Inputs mode/preset/baseline-runner/candidate-runner/seed/cases/scenario-family/baseline-args/candidate-args; outputs status/report-path/discovery-path.
- [ ] Write failing structural validation tests asserting metadata and the artifact uploader.
- [ ] Implement Action steps: pinned Rust, conditional build, script invocation with continue-on-error, pinned upload-artifact, final exit status. Ensure snapshot compatibility.
- [ ] Add a CI job for preset self-comparison and controlled custom difference with status/evidence assertions.
- [ ] Verify local structural tests, GitHub Actions jobs and Rust/Python suites; commit.

### Task 3: Adoption documentation
**Files:** Modify docs/ci-integration.md, README.md and docs/roadmap.md.
- [ ] Add examples of instant preset and custom runner workflows, with explicit negative-control scope.
- [ ] Review guidance against actual Action inputs; run docs/site tests; commit.
- [ ] Open PR, await required CI, then merge. Keep release version decision separate.

## Verification
python3 -m unittest discover -s scripts/tests -v
cargo +1.95.0 fmt --check
cargo +1.95.0 test --locked --workspace --all-targets
cargo +1.95.0 clippy --locked --workspace --all-targets -- -D warnings
node --test site/tests/site.test.mjs
GitHub Action smoke job for both differential modes and snapshot regression.
