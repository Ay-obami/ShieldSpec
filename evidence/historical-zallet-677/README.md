# Reproduced Zallet #677 privacy-policy regression (historical)

This directory contains a **real, reproducible difference in privacy-policy behavior** across two revisions of [zcash/zallet pull #677](https://github.com/zcash/zallet/pull/677). The affected version accepts an Ironwood-input → Sapling-change proposal under `FullPrivacy`; the corrected version rejects it. Crossing shielded pools discloses a value balance on-chain. This is a **previously reported and fixed Zcash wallet-core issue**, not a newly discovered issue in upstream software.

This is an independent, **test-only reproduction using the real Zallet policy implementation**. It is **not** an automatically discovered failure of ShieldSpec's general-purpose librustzcash differential fuzzer. The accompanying ShieldSpec CLI demo exercises that tool separately with an explicitly injected fault. Zallet is Zcash wallet core, not a claim that these two commits are librustzcash SDK releases.

## Exact revisions

| | Zallet Git revision | Witness result |
| --- | --- | --- |
| Before fix | `f496c9b6b2c6ebb0065b329c11bda4a5fdf73568` | `Ironwood→Sapling FullPrivacy accepted=true`; 2 control tests pass, expected regression assertion fails |
| With fix | `e966780216b7a86b0a1a6618f2fde5b574da7171` | `Ironwood→Sapling FullPrivacy accepted=false`; 3 tests pass |

The identical [Rust witness](shieldspec_privacy_history.rs) is injected as a `#[cfg(test)]` module in both checkouts; **no production privacy-policy logic is modified**. The proposal is a real `zcash_client_backend::Proposal`, using a 20,000-zat Ironwood V3 note, 10,000-zat Sapling change and 10,000-zat fee. The witness calls Zallet's actual `enforce_privacy_policy` function, not a substitute JSON evaluator. No broadcast or live funds are involved.

Two controls protect the interpretation: Orchard→Sapling is rejected on both versions, and Ironwood→Ironwood is allowed on both. The before test intentionally exits 101 because the regression assertion catches the erroneous acceptance. That nonzero exit is the expected historical signal, **not a compilation failure**.

## Reproduce

With Rust **1.95.0**, Git, Python 3, network access, and enough disk for compiling historical upstream wallet dependencies:

```bash
python3 -m unittest discover -s scripts/tests -p test_historical_zallet_reproduction.py -v
python3 scripts/reproduce_zallet_677.py \
  --output evidence/historical-zallet-677/results
```

For already checked-out pinned trees, use `--before PATH --after PATH` (both required) and optionally `--cargo-target-dir PATH`. The harness refuses unexpected production changes, records hashes of the exact Rust witness and complete before/after logs, and exits nonzero if the observed behavior does not match the two controls and the privacy property.

## Evidence

- [Machine-readable verdict](results/verdict.json) — pinned revisions, Rust/Cargo versions, per-revision compiled test-runner SHA-256, exact command, one-step minimized-by-construction fixture with fixed test vectors, input parameters, results, log hashes, witness source hash and explicit claim limits. This particular historical test witness is manually minimal; no automated upstream-case shrinking is claimed.
- [Affected revision raw test log](results/before.log) — `accepted=true`, two passing controls and the expected failed privacy assertion.
- [Corrected revision raw test log](results/after.log) — `accepted=false`, all three tests passing.
- [Upstream Zallet #677](https://github.com/zcash/zallet/pull/677) — the original documented hardening change.
- [Zallet #693](https://github.com/zcash/zallet/issues/693) — context about privacy-policy test construction.

## Scope

- **Historical and corrected privacy-policy difference:** independently reproduced.
- **Newly discovered zero-day or undocumented issue:** not claimed.
- **Auto-generated ShieldSpec fuzzer discovery of this issue:** not claimed.
- **Fresh real-world wallet transaction:** not claimed.
- **Comparison of zallet-core policy functions:** yes; separate from the independent [librustzcash #2614 correctness reproduction](../historical-2614/README.md).
