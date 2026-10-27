# ShieldSpec — Zecathon Core & Tooling submission

## Project title

**ShieldSpec — Privacy Differential Fuzzer for Zcash Wallet Proposals**

## Two-sentence description

ShieldSpec generates realistic Zcash wallet states, exercises actual SDK transaction-proposal logic and checks the resulting proposal against explicit privacy invariants. It compares runner builds, minimizes surprising cases into deterministic replays and integrates those checks into development CI, before anything gets signed or broadcast.

## Why it exists

ShieldSpec addresses testing gaps documented in Zcash's own engineering work:

- [zallet#693](https://github.com/zcash/zallet/issues/693): realistic proposal state was cumbersome to construct for privacy-policy unit tests (some ad hoc tests have since landed).
- [librustzcash#2614](https://github.com/zcash/librustzcash/issues/2614): proposal edge cases evaded existing tests, including a transparent send-max failure.
- [librustzcash#2775](https://github.com/zcash/librustzcash/issues/2775): developers requested explicit metadata leak-regression assertions where correctness tests did not cover the privacy property.

These issues provide context, not the centerpiece. ShieldSpec does not claim to close them or to discover an undisclosed upstream vulnerability.

## What actually works

1. **Real SDK proposal execution:** isolated wallet state; Sapling/Orchard funding; synthetic block scanning; genuine `librustzcash` proposals.
2. **Privacy-aware differential:** compare identical scenarios across separate executable builds; report only new or worsened policy findings.
3. **Minimization and replay:** save original and reduced scenario, seed, ordinal, policy, runner arguments and binary SHA-256; verify executable identity at replay.
4. **ZIP-320 correctness:** recognize legitimate ephemeral transparent bridge outputs in real shielded-to-TEX proposals.
5. **Developer integration:** Rust libraries, CLI, SARIF/JSON/human reporters, composite GitHub Action and CI tests.

## Genuine historical Zcash SDK difference

We executed the same upstream test `send_max_spendable_to_transparent` against two real Git commits:

| Affected SDK | Corrected SDK |
| --- | --- |
| `54ff673169ec8be5a4b023a7ae69bcf5caead2a3` | `25766a4c754a847b0a9e3a63801d8f46a270c01b` |
| `Proposal(PaymentPoolsMismatch)` | Same wallet case passes, 1 test, 0 failures |

The affected checkout received **only the exact newly added upstream regression test and its trivial test registrations**; the production fix was not copied. See [test SHA-256, raw logs and verdict](../evidence/historical-2614/README.md). **This is SDK correctness evidence, not a privacy finding.**

## ShieldSpec's privacy-differential demo

```bash
cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- demo --format human
```

The baseline and candidate execute real SDK proposals. The candidate then receives a **deliberately injected post-normalization transparent-change fault**, making the complete detection, shrink and replay loop reproducible. The result contains `SS003_TRANSPARENT_CHANGE` and `SS005_PRIVACY_BOUNDARY` and exits code `2`. We do not represent that injected fault as an upstream bug.

```bash
cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- \
  replay --seed 148 --ordinal 0 --policy standard \
  --demo-regression transparent-change --format human
```

## Reproduce and audit

```bash
cargo fmt --check
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
python3 -m unittest discover -s scripts/tests -v
python3 scripts/reproduce_librustzcash_2614.py --output evidence/historical-2614/results
```

The last command compiles two old upstream SDK checkouts and therefore takes longer. A manual GitHub Actions workflow runs it and uploads complete logs.

## Evidence limits

- Never handles seed phrases, signing, mainnet custody or broadcast.
- Wallet chain inputs are synthetic; SDK proposal construction is real.
- No inference of anonymity or guarantee that all privacy defects are covered.
- No novel upstream privacy finding claimed.
- User-declared SDK revision labels require external verification; binary hashes and the pinned historical Git checkouts have verifiable identities.

## Final delivery

A [captioned 1920×1080 demo video](media/shieldspec-zecathon-captioned-demo.mp4) has been generated from checked historical logs and executed CLI output. It is approximately 2:20 long, **without voiceover**, and it explicitly separates the real historical correctness difference from the deliberate privacy fault. Regenerate using `python3 scripts/render_demo_video.py` after `cargo build --locked -p shieldspec-zcash-runner --bins` (requires Pillow and ffmpeg). Captured output and a SHA-256 manifest are written to ignored `demo-artifacts/`. For a narrated screen recording, use the [voiceover and shot list](demo-video-script.md). Before external submission, confirm a public video URL, check that CI is green and submit on the hackathon website; repository publication alone is not an accepted entry.
