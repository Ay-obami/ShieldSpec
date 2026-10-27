# ShieldSpec — Zcash Privacy Regression TestKit Design

**Date:** 2026-10-27
**Track:** ZECATHON Core & Tooling
**Intent:** Give Zcash wallet developers repeatable tests that catch privacy regressions in transaction proposals before code ships.

## Why

A transaction can remain valid while a wallet change introduces a new transparent input, transparent output, transparent change, unexpected receiver selection, or privacy-sensitive migration. Existing wallet guidance describes privacy policies, but developers still need practical proposal builders, assertions, regression comparison, and CI output.

ShieldSpec turns privacy expectations into tests.

## Goals

- Analyze locally available Zcash transaction-proposal facts before signing.
- Detect confirmed proposal-level privacy exposures, not infer hidden chain data.
- Compare a candidate proposal against a known-good baseline and fail when privacy gets worse.
- Provide a small builder API so wallet developers can construct realistic privacy test cases.
- Expose human, JSON, and SARIF output.
- Run from a CLI and a reusable GitHub Action.
- Demonstrate at least one privacy regression that a destination-address-only check misses.
- Keep the engine reusable as a Rust library.

## Non-goals / anti-goals

- No mainnet signing, broadcasting, custody, seed phrases, or spending keys.
- No numeric anonymity score or claim that absence of findings guarantees privacy.
- No attempt to deanonymize shielded transactions from public chain data.
- No blocking users at runtime; this project targets developer testing and CI.
- No claim of being the first privacy tool; originality is proposal-aware regression testing and developer ergonomics.

## Architecture

Rust workspace:

- `shieldspec-core`: normalized proposal snapshot, policy engine, findings, regression comparison.
- `shieldspec-testkit`: ergonomic proposal/step builders and canonical privacy fixtures.
- `shieldspec-zcash`: adapter from real `zcash_client_backend::proposal::Proposal` structures into the normalized snapshot.
- `shieldspec-cli`: `check`, `diff`, and fixture commands with human/JSON/SARIF reporters.
- root `action.yml`: GitHub composite action invoking the CLI.
- `examples/mock-wallet`: independent consumer crate demonstrating library integration.

## Locked public interfaces

Core model:

- `Pool::{Transparent,Sapling,Orchard,Ironwood,Unknown}`
- `OutputRole::{Recipient,Change,Other}`
- `InputFact { pool, value_zat }`
- `OutputFact { pool, role, value_zat }`
- `StepSnapshot { index, inputs, outputs, fee_zat }`
- `ProposalSnapshot { schema_version, network, steps }`
- `Severity::{Info,Warning,Error}`
- `Finding { rule_id, severity, step_index, message, evidence, fingerprint }`
- `Policy::{standard(),strict()}`
- `Assessment { findings, passes }`
- `assess(snapshot, policy) -> Assessment`
- `compare(baseline, candidate, policy) -> RegressionReport`

Initial rule IDs:

- `SS001_TRANSPARENT_RECIPIENT`
- `SS002_TRANSPARENT_FUNDING`
- `SS003_TRANSPARENT_CHANGE`
- `SS004_UNKNOWN_ROUTE`
- `SS005_PRIVACY_BOUNDARY`

A finding fingerprint must be deterministic and based on rule + step + structural evidence, not free-form text.

Regression semantics:

- A candidate regresses when it adds a policy-failing finding not present in the baseline, or worsens severity for the same structural finding.
- Value-only differences do not create a new structural finding unless they cross a privacy boundary.
- Exit codes: 0 = policy passes/no regression; 2 = privacy failure/regression; 1 = operational/input error.

CLI contract:

- `shieldspec check --input proposal.json --policy standard|strict --format human|json|sarif`
- `shieldspec diff --baseline baseline.json --candidate candidate.json --policy standard|strict --format human|json|sarif`
- `shieldspec fixture list`
- `shieldspec fixture write <name> <path>`

Required fixtures:

- `shielded-payment`
- `transparent-recipient`
- `transparent-funding`
- `transparent-change`
- `unified-selected-transparent`
- `multi-step-late-leak`
- `privacy-preserving-migration`
- `full-balance-migration`

## Policy behavior

Standard policy:
- Transparent recipient: Warning, fail check.
- Transparent funding: Warning, fail check.
- Transparent change: Error, fail check.
- Unknown route: Warning, fail closed.
- Privacy boundary transition: Warning, fail check.

Strict policy upgrades all privacy exposures to Error.

## GitHub Action

The root action accepts:
- `candidate` (required)
- `baseline` (optional; when present runs diff)
- `policy` default `standard`
- `format` default `human`

The action must never require wallet secrets. It operates only on committed/generated proposal snapshots.

## Demo / magic moment

Demo a wallet PR whose recipient still looks shielded but whose candidate proposal introduces transparent change. The baseline passes; `shieldspec diff` fails and points at `SS003_TRANSPARENT_CHANGE`. Then show the same failure in GitHub Actions / SARIF.

## Acceptance criteria

- Workspace format, tests, and strict Clippy pass.
- Core unit tests cover every rule and multi-step proposals.
- Regression tests prove a new late-step leak fails.
- Property tests cover deterministic fingerprints and monotonic exposure behavior.
- Zcash adapter compiles against the same modern SDK family already used successfully by LeakLens.
- CLI human/JSON/SARIF outputs are deterministic and snapshot-tested.
- GitHub Action smoke test runs in CI without secrets.
- Example consumer crate compiles and asserts a privacy regression.
- README can explain the product in two sentences and reproduce the demo in under five minutes.

## Approaches considered

1. Extend LeakLens consumer UI — rejected because it keeps the product centered on generic warnings and hides the strongest developer-facing technical contribution.
2. New standalone ShieldSpec repo — selected because it gives the hackathon a clean Core & Tooling artifact, issue history, CI story, and reusable library surface.
3. Patch an upstream wallet directly — rejected for the sprint because it couples delivery to another repository and reviewer cycle.

## Builder calls

- Use Rust stable and serde-compatible JSON snapshots.
- Prefer deterministic, explainable rules over scoring.
- Reuse concepts from LeakLens only where they match this spec; do not copy its product framing.
- Every production behavior follows red-green-refactor TDD.
