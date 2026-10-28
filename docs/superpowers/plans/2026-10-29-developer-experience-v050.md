# ShieldSpec v0.5 Developer Experience Implementation Plan

> **For agentic workers:** Use test-driven development and verification-before-completion; preserve all advanced commands and existing result semantics.

## Goal
Make everyday ShieldSpec workflows intuitive without requiring developers to copy long runner commands. Level 3 automatic wallet-runner generation is explicitly out of scope.

## Approach
Add small, backward-compatible CLI conveniences around the current TOML project configuration and existing independently built fuzzer. Prefer direct commands with defaults over generating arbitrary wallet adapters; retain exact historical command lines in advanced reproduction guides.

## Task 1 — CLI behavior (owner: CLI agent)
- Add `shieldspec init` that writes a safe valid starter snapshot config and local sample fixtures, refuses overwrite, and clearly identifies built-in negative-control status.
- Add `shieldspec test --compare` selecting configured differential mode with explicit errors for absent runners.
- Add `shieldspec replay FILE` that forwards to existing fuzzer `replay --input FILE`, preserving the fuzzer's hash/argument verification and exit status semantics.
- Preserve `try`, `test`, `check`, `diff`, `fixture`, and existing 0/1/2 behavior.

## Task 2 — Test behavior (owner: tests agent)
- Write failing CLI integration tests before Task 1 production edits.
- Test non-destructive initialization, default usage, differential failures, replay delegation, and old commands.
- Update site tests to match documented short commands. Run targeted and full relevant suites.

## Task 3 — Documentation (owner: docs agent)
- Lead README, website, getting-started, and demo pages with install/init/test/compare/replay workflow.
- Identify fuzzer binary and runner source-build requirements honestly, and preserve historical exact commands in advanced/provenance sections.
- Clearly distinguish bundled self-controls from independent wallet comparison and retain Level 3 deferred note.

## Task 4 — Integration (owner: main agent)
- Check concurrent file edits and resolve mismatches.
- Review versioning and update CLI crate version / SARIF golden and lockfile only if release policy warrants it.
- Run Rust workspace tests, Clippy, site tests, Cargo packaging dry run, command smoke tests, and git diff check.
- Push new feature branch, open PR, and report CI; merge only after checks are green and within authorization.
