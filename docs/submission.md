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
2. **Broad real-wallet coverage:** Sapling recipients, mixed Unified Address receiver preference, shielded change, wallet-owned transparent-UTXO shielding, ZIP-317 fees, note/input selection and NU6.3/Ironwood boundaries (merged [PR #43](https://github.com/Ay-obami/ShieldSpec/pull/43)).
3. **Privacy-aware differential:** compare identical scenarios across separate executable builds; compare normalized findings and matching observed wallet-policy verdicts while separating execution errors from policy decisions.
4. **Minimization and replay:** save original and reduced scenario, seed, ordinal, policy, runner arguments and binary SHA-256; verify executable identity at replay.
5. **ZIP-320 correctness:** recognize legitimate ephemeral transparent bridge outputs in real shielded-to-TEX proposals.
6. **Developer integration:** Rust libraries, CLI, SARIF/JSON/human reporters, composite GitHub Action and CI tests. The extended real-wallet feature branch passed 87 workspace tests before merge.

## Genuine historical Zcash privacy-policy difference

An identical test-only witness creates a real Zcash wallet proposal and calls Zallet's actual `enforce_privacy_policy` with `FullPrivacy` on two pinned `zcash/zallet` revisions around [upstream hardening PR #677](https://github.com/zcash/zallet/pull/677):

| Scenario | Before fix `f496c9b6` | After fix `e9667802` |
| --- | --- | --- |
| **Ironwood note → Sapling change** | `accepted=true` — public cross-pool value balance incorrectly allowed | `accepted=false` — policy guard works |
| Orchard note → Sapling change (control) | Rejected | Rejected |
| Ironwood note → Ironwood change (control) | Accepted | Accepted |
| Actual `cargo test` | Exit 101: 2 pass, 1 deliberately failing privacy assertion | Exit 0: all 3 pass |

The [pinned source, exact full revisions, test SHA-256, reproducibility command, raw logs and verdict](../evidence/historical-zallet-677/README.md) are supplied. **This is an independently reproduced, previously disclosed and fixed Zcash wallet-core bug**, not a newly discovered flaw and not a claim that the general ShieldSpec fuzzer automatically found it. No production code was patched in either checkout.

Separately, the [librustzcash#2614 historical SDK test](../evidence/historical-2614/README.md) reproduces a transparent send-max **correctness**, rather than privacy, difference (`PaymentPoolsMismatch` before fix; passing afterward). The expanded corpus also exercises the documented [librustzcash#3133](https://github.com/zcash/librustzcash/issues/3133) mixed Sapling/Orchard input-selection case as an availability/correctness regression, **not** a newly discovered privacy leak. These are supporting engineering context, not the historical Zallet privacy proof.

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

An earlier [captioned 1920×1080 silent demo](media/shieldspec-zecathon-captioned-demo.mp4) remains committed as reproducible evidence of the historical Zallet comparison and deliberately injected ShieldSpec CLI fault. Regenerate it using `python3 scripts/render_demo_video.py` after `cargo build --locked -p shieldspec-zcash-runner --bins` (requires Pillow and ffmpeg); captured output and a SHA-256 manifest go to ignored `demo-artifacts/`.

A **new female-voiced walkthrough** was produced locally from genuine browser interaction—not a stitched screenshot slideshow—and independently muxed into `/home/ayobami/Videos/ShieldSpec-Zecathon-2026-Voiced-Demo-20261028.mp4` (H.264/AAC, about 109.7 seconds). It starts on the public landing page, interacts with historical evidence tabs and developer sections, shows real GitHub PR/code context, and opens upstream Zcash issues #677, #693, #2614 and #2775. This MP4 is **local only** as of this update: no public video URL or hackathon upload is confirmed. Before external submission, verify the final audio/video playback, publish an accessible video link, check CI, and submit via the hackathon form; publishing source alone is not an accepted entry.
