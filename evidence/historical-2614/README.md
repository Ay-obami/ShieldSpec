# Verified historical `librustzcash` behavior change

This is **genuine historical SDK execution evidence**, not a claim that ShieldSpec found an undisclosed bug or a new privacy leak.

- Upstream report: [zcash/librustzcash#2614](https://github.com/zcash/librustzcash/issues/2614)
- Upstream merged fix: [zcash/librustzcash#2615](https://github.com/zcash/librustzcash/pull/2615)
- Affected SDK Git revision: [`54ff673169ec8be5a4b023a7ae69bcf5caead2a3`](https://github.com/zcash/librustzcash/commit/54ff673169ec8be5a4b023a7ae69bcf5caead2a3)
- Corrected SDK Git revision: [`25766a4c754a847b0a9e3a63801d8f46a270c01b`](https://github.com/zcash/librustzcash/commit/25766a4c754a847b0a9e3a63801d8f46a270c01b)

## Scenario

The upstream regression test `wallet::sapling::tests::send_max_spendable_to_transparent`:

1. Creates a real isolated Zcash test wallet with **one 60,000-zat Sapling note**.
2. Requests **send-max** to an ordinary transparent P2PKH recipient using ZIP-317 fees.
3. Asserts a valid single-step proposal with transparent payment and correct fee; it then builds the transaction.

The pre-fix checkout did not yet have this test. The reproducer **copies only the exact upstream test function and the minimal registration wrappers** from the post-fix source into that checkout. It does not backport the actual fixing implementation. It also checks the Git HEAD in each worktree and records SHA-256 of the test function used.

## Actual result — 27 October 2026

| Commit | Exit code | Observable result |
| --- | ---: | --- |
| `54ff673169ec` (before) | 101 | `.unwrap()` panics on `Proposal(PaymentPoolsMismatch)` |
| `25766a4c754a` (after) | 0 | The same test passes, `1 passed; 0 failed` |

The machine-readable evidence is [`results/verdict.json`](results/verdict.json), accompanied by untouched [`results/before.log`](results/before.log) and [`results/after.log`](results/after.log).

Reproduce from a clean ShieldSpec checkout with Rust 1.95, Python 3, and Git:

```bash
python3 -m unittest discover -s scripts/tests -v
python3 scripts/reproduce_librustzcash_2614.py \
  --output evidence/historical-2614/results
```

It can also run as an on-demand GitHub Actions job, `Historical librustzcash #2614 reproduction`, which preserves all three evidence artifacts.

## Evidence boundary

This is a real, documented **correctness regression** in the SDK's `propose_send_max_transfer` proposal path. It is not a privacy regression, and it is not the main ShieldSpec fuzzing demo. It illustrates why generating realistic wallet states and running actual SDK proposal logic matters. ShieldSpec's privacy-specific detection is demonstrated separately via its clearly labeled injected change-pool fault, seed-based minimization and replay, and ZIP-320 semantics test.

Another independent, **correctness/availability** example is [librustzcash #3133](https://github.com/zcash/librustzcash/issues/3133), which ShieldSpec now reproduces with a pinned real SDK mixed-Sapling/Orchard input-selection case and a successful control in [`tests/mixed_pool.rs`](../../crates/shieldspec-zcash-runner/tests/mixed_pool.rs), merged via [ShieldSpec PR #43](https://github.com/Ay-obami/ShieldSpec/pull/43). Neither #2614 nor #3133 should be described as an upstream privacy leak.
