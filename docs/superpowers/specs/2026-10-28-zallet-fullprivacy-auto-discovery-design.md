# Real Historical Zallet Privacy-Policy Differential — Design

Date: 2026-10-28
Status: Approved for implementation by the user ("yes" to completing genuine dual-Zallet-runner integration).

## Intent
Make ShieldSpec's general differential engine generate Zallet-compatible historical privacy-policy proposal scenarios and automatically detect the documented FullPrivacy difference between two independently compiled, pinned Zallet policy implementations. It must not substitute stored verdict JSON for live wallet-policy execution.

## Proven basis
- Existing test-only witness: `evidence/historical-zallet-677/shieldspec_privacy_history.rs`.
- Affected Zallet revision: `f496c9b6b2c6ebb0065b329c11bda4a5fdf73568`.
- Corrected Zallet revision: `e966780216b7a86b0a1a6618f2fde5b574da7171`.
- Existing general engine: `crates/shieldspec-fuzz` plus `shieldspec-fuzz fuzz/replay`, process protocol v1.
- Historically demonstrated: Ironwood V3 input + Sapling change accepted by affected FullPrivacy, rejected by corrected FullPrivacy, two shared controls.

## Chosen approach and alternatives
1. **Chosen: real policy probe.** A dedicated `zallet_privacy_probe` scenario family carries pool and balanced-value inputs. Identical test-only Rust probe source is injected into both pinned Zallet revisions, constructs a real `Proposal`, invokes real `enforce_privacy_policy`, and prints machine-readable observed facts. A process adapter converts those real observations into ShieldSpec's existing runner JSON protocol.
2. **Rejected: replay captured before/after verdict JSON.** It would report a known answer without executing upstream policy and would not prove automatic detection.
3. **Deferred: wallet daemon/network transaction integration.** Unnecessary for testing `FullPrivacy` against real in-memory proposal objects, adds security and operational risk.

## User-visible result
Run `shieldspec-fuzz fuzz --scenario-family zallet-privacy --seed 2026 --cases 3` with corrected-policy reference runner as baseline and historical affected-policy runner as candidate. The engine must record `wallet_policy_difference` when the generated Ironwood-to-Sapling case reaches both real policies. Replay from the recorded discovery reproduces the difference; shrink must retain a valid balanced scenario. The comparison direction must be explicitly labeled historical corrected-reference versus affected revision, not chronological new-regression discovery.

## Scenario contract
`{"kind":"zallet_privacy_probe","params":{"input_pool":"ironwood","change_pool":"sapling","input_zat":20000,"fee_zat":10000}}`
- `input_pool`: `ironwood` or `orchard`; `change_pool`: `ironwood`, `orchard`, or `sapling`.
- Values are positive Zatoshis, `fee_zat < input_zat`, `change_zat = input_zat - fee_zat > 0`. There is no recipient payment in this probe: it observes a one-input, one-change, fee-bearing real Proposal.
- A deterministic three-case window must include Ironwood→Ironwood allowed on both, Orchard→Sapling rejected on both, Ironwood→Sapling difference. Seed changes valid values; ordinal selects pool pair.
- Invalid schema, unsupported pools, arithmetic overflow, malformed runner output, missing actual probe marker, process failure or wrong binary hash must fail closed without inventing a wallet rejection.
- Both runners must return a comparable policy name `zallet_full_privacy_v1` and real `accepted`/`rejected` decision via protocol v1 `status: observed` with a complete normalized one-step snapshot tied to the constructed Proposal.

## Truth and provenance
- Never patch Zallet production policy logic. Inject identical test-only code at the same module boundary and record source SHA.
- Persist both compiled test executables separately and hash their bytes. Enforce manifest/binary hashes on invocation and replay, recording source revisions and original/minimized generated scenarios.
- Positive divergence must come from live real Zallet code, not from hand-encoded branch-specific conditions.
- Historical issue is previously documented/fixed; no claim of novel upstream vulnerability or broadcast transaction.
- Keep the already-approved V2 demo, site, merged main and existing historical witness unchanged until independently verified.

## Verification
- Unit tests for generator/serde/shrink, adapter protocol errors, hashing and marker parsing.
- Real compiled dual-revision test: paired controls agree, generated Ironwood→Sapling differs, generic engine records a policy difference and replay reproduces it.
- Cargo format/tests/lint, Python tests, CI reproducibility artifacts.
- Build may require fresh Zallet compilation; share Cargo cache and copy each executable before the next historical build to keep disk usage bounded.
