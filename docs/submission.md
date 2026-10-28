# ShieldSpec — archived 2026 Zecathon submission notes

This is a historical record of ShieldSpec's Core & Tooling hackathon submission, **not the current product homepage or installation guide**. The submission and final video upload were completed. For maintained commands and limitations, start with the [README](../README.md) and [getting-started guide](getting-started.md); for independently repeatable source-level proof, see [Zallet auto-discovery](zallet-auto-discovery.md).

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
7. **Browsable explanations and on-site developer documentation:** the [ShieldSpec website](https://ay-obami.github.io/ShieldSpec/) explains the problem, supported checks, engine workflow and real historic evidence, while the [developer guide](https://ay-obami.github.io/ShieldSpec/docs.html) provides copyable setup, snapshot checks, real SDK comparison, historical runner reproduction, exit codes and CI integration. The site is a read-only explainer, not a wallet or a live browser-side fuzzer.

## Genuine historical Zcash privacy-policy difference

An identical test-only witness creates a real Zcash wallet proposal and calls Zallet's actual `enforce_privacy_policy` with `FullPrivacy` on two pinned `zcash/zallet` revisions around [upstream hardening PR #677](https://github.com/zcash/zallet/pull/677):

| Scenario | Before fix `f496c9b6` | After fix `e9667802` |
| --- | --- | --- |
| **Ironwood note → Sapling change** | `accepted=true` — public cross-pool value balance incorrectly allowed | `accepted=false` — policy guard works |
| Orchard note → Sapling change (control) | Rejected | Rejected |
| Ironwood note → Ironwood change (control) | Accepted | Accepted |
| Actual `cargo test` | Exit 101: 2 pass, 1 deliberately failing privacy assertion | Exit 0: all 3 pass |

The [original pinned historical witness, exact Git revisions, test hashes, raw logs and verdict](../evidence/historical-zallet-677/README.md) remain preserved. **New technical proof:** ShieldSpec's normal differential fuzzer now independently executes two compiled Zallet `FullPrivacy` policy runners through an explicit dynamic scenario family. In a verified three-case seed-2026 run, both control pairs agreed; ordinal 2 produced `wallet_policy_difference`, with the corrected baseline rejecting and historically affected candidate accepting. The shrinker reduced the true proposal from 102,837 input / 11,590 fee to **2 input / 1 fee** without losing the actual policy difference; saved replay reported `wallet_policy_difference_reproduced`, `recorded_rule_reproduced: true` and `provenance_verified: true`. [Run and audit the exact workflow](zallet-auto-discovery.md). This rediscovered **previously disclosed and fixed** Zcash wallet-core behavior; it is not a novel vulnerability. Neither checkout's production privacy logic was modified.

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
# For the independently compiled general-fuzzer discovery and replay:
# see docs/zallet-auto-discovery.md
python3 scripts/reproduce_librustzcash_2614.py --output evidence/historical-2614/results
```

The last two commands compile pinned upstream wallet/SDK checkouts and therefore take longer. Separate manual GitHub Actions workflows can rerun each one and upload complete logs.

## Evidence limits

- Never handles seed phrases, signing, mainnet custody or broadcast.
- Wallet chain inputs are synthetic; SDK proposal construction is real.
- No inference of anonymity or guarantee that all privacy defects are covered.
- No novel upstream privacy finding claimed.
- User-declared SDK revision labels require external verification; binary hashes and the pinned historical Git checkouts have verifiable identities.

## Historical delivery

The final walkthrough was uploaded to YouTube and the hackathon submission was completed on October 28, 2026. The video is not stored in Git; no public URL is recorded in these notes. The [recording storyboard](demo-video-script.md) is retained as historical production context, not an outstanding delivery task.

Reproducibility does **not** depend on a video. Use the executable commands above, the committed [historical evidence](../evidence/historical-zallet-677/README.md), or the [independent-runner discovery recipe](zallet-auto-discovery.md) to audit the results. To regenerate a local demonstration video, `scripts/render_demo_video.py` requires a built `shieldspec-zcash-runner`, Pillow and ffmpeg; it writes ignored artifacts under `demo-artifacts/`.
