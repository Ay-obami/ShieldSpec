# ShieldSpec — Zecathon Core & Tooling submission

## Project title

**ShieldSpec — Privacy Differential Fuzzer for Zcash Wallet Proposals**

## Two-sentence description

ShieldSpec generates realistic Zcash wallet states, exercises actual SDK transaction-proposal logic and checks the resulting proposal against explicit privacy invariants. It compares runner builds, minimizes surprising cases into deterministic replays and integrates those checks into development CI, before anything gets signed or broadcast.

## Why it exists

ShieldSpec addresses testing gaps documented in Zcash's own engineering work:

- [zallet#677](https://github.com/zcash/zallet/pull/677): a documented `FullPrivacy` bug allowed cross-pool value-balance exposure for newer shielded pools such as Ironwood; ShieldSpec independently replays the original behavior and its fix against actual wallet policy code.
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

## Genuine historical Zcash privacy-policy difference

An identical test-only witness creates a real Zcash wallet proposal and calls Zallet's actual `enforce_privacy_policy` with `FullPrivacy` on two pinned `zcash/zallet` revisions around [upstream hardening PR #677](https://github.com/zcash/zallet/pull/677):

| Scenario | Before fix `f496c9b6` | After fix `e9667802` |
| --- | --- | --- |
| **Ironwood note → Sapling change** | `accepted=true` — public cross-pool value balance incorrectly allowed | `accepted=false` — policy guard works |
| Orchard note → Sapling change (control) | Rejected | Rejected |
| Ironwood note → Ironwood change (control) | Accepted | Accepted |
| Actual `cargo test` | Exit 101: 2 pass, 1 deliberately failing privacy assertion | Exit 0: all 3 pass |

The [pinned source, exact full revisions, test SHA-256, reproducibility command, raw logs and verdict](../evidence/historical-zallet-677/README.md) are supplied. **This is an independently reproduced, previously disclosed and fixed Zcash wallet-core bug**, not a newly discovered flaw and not a claim that the general ShieldSpec fuzzer automatically found it. No production code was patched in either checkout.

Separately, the [librustzcash#2614 historical SDK test](../evidence/historical-2614/README.md) reproduces a transparent send-max **correctness**, rather than privacy, difference (`PaymentPoolsMismatch` before fix; passing afterward). We retain it as additional engineering context, not the privacy proof.

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
python3 scripts/reproduce_zallet_677.py --output evidence/historical-zallet-677/results
python3 scripts/reproduce_librustzcash_2614.py --output evidence/historical-2614/results
```

The last two commands compile pinned upstream wallet/SDK checkouts and therefore take longer. Separate manual GitHub Actions workflows can rerun each one and upload complete logs.

## Evidence limits

- Never handles seed phrases, signing, mainnet custody or broadcast.
- Wallet chain inputs are synthetic; SDK proposal construction is real.
- No inference of anonymity or guarantee that all privacy defects are covered.
- No novel upstream privacy finding claimed.
- User-declared SDK revision labels require external verification; binary hashes and the pinned historical Git checkouts have verifiable identities.

## Final delivery

A [captioned 1920×1080 demo video](media/shieldspec-zecathon-captioned-demo.mp4) is generated from checked before/after Zallet privacy-policy logs and executed ShieldSpec CLI output. It is approximately 2:20 long, **without voiceover**, and it explicitly separates the genuine historical wallet-policy privacy difference from the deliberately injected ShieldSpec CLI fault. Regenerate using `python3 scripts/render_demo_video.py` after `cargo build --locked -p shieldspec-zcash-runner --bins` (requires Pillow and ffmpeg). Captured output and a SHA-256 manifest are written to ignored `demo-artifacts/`. For a narrated screen recording, use the [voiceover and shot list](demo-video-script.md). Before external submission, confirm a public video URL, check that CI is green and submit on the hackathon website; repository publication alone is not an accepted entry.
