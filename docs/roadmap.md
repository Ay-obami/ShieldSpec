# ShieldSpec product roadmap

ShieldSpec is **reusable privacy-regression infrastructure for Zcash wallet developers**, not a one-off vulnerability claim. This page distinguishes code already merged into `main` from features needing further wallet integration. The [public issue tracker](https://github.com/Ay-obami/ShieldSpec/issues) and linked PRs hold review details; the historical `v0.1.0` tag predates the expanded fuzzer; the `v0.2.0` source release includes it and four workspace crates are also published to crates.io, while the composite GitHub Action remains snapshot-only.

## Available on main

| Capability | Boundary |
| --- | --- |
| Privacy policy core and normalized proposal model | Explicit proposal-level transparent exposure, routing and related invariants |
| Snapshot CLI | `check`, `diff`, human/JSON/SARIF and 0/1/2 exit status |
| GitHub Action (`v0.2.0`; older `v0.1.0`) | Baseline-versus-candidate snapshot privacy check; does **not** spin up a wallet |
| Deterministic real proposal harness and controlled demo | Isolated synthetic chain and wallet state; real Zcash SDK proposal construction, baseline/candidate processes, replay and deliberate fault injection |

The [GitHub Pages site](https://ay-obami.github.io/ShieldSpec/) is publicly reachable and its `site/` source plus historical Zallet evidence are now in `main`. The site is an **interactive read-only explorer**, not a browser-hosted SDK fuzzer. New site copy will only become public after the corresponding GitHub Pages deployment succeeds.

None of these tools proves wallet anonymity. Historical upstream behavior must not be described as a ShieldSpec-discovered zero-day.

## Merged on `main` — generic policy comparison and broader real-wallet coverage

[PR #42 — observed wallet-policy decisions and Ironwood scenarios](https://github.com/Ay-obami/ShieldSpec/pull/42) and [PR #43 — broader real-wallet privacy-policy coverage](https://github.com/Ay-obami/ShieldSpec/pull/43) are merged. These capabilities can be built and tested from a fresh `main` clone. The `v0.2.0` GitHub Action still runs only the lightweight snapshot gate:

| Capability | Verified scope and limits |
| --- | --- |
| `SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE` | Detects aggregate shielded-pool public value balance crossings in real normalized proposals; **not** proof of a wallet-policy violation |
| First-class policy-verdict runner protocol | Optional named `accepted`/`rejected` observation alongside the full proposal snapshot, preserving operational errors separately |
| Generic verdict differential | Comparable policy names, decision differences, minimization, provenance recording, saved-case replay and coverage counts |
| Real SDK spend-policy observation | Opt-in `--wallet-spend-policy orchard-only` invokes actual pinned SDK Orchard-only source restriction; it is **not Zallet FullPrivacy** |
| Activation-aware scenario family | `--scenario-family ironwood --cases 4` covers Orchard/Sapling funding before/at NU6.3, including real Orchard→Ironwood and Sapling→Ironwood proposals |
| Provenance and replay safeguards | Runner SHA-256 and exact args verified on saved replay; SDK labels are user-declared, not hash-to-source proof |
| Expanded real-wallet proposal cases (PR #43) | Real Sapling recipient proposals, mixed Unified Address receiver preference, shielded change behavior, wallet-owned transparent UTXO shielding, ZIP-317 fees and input/note selection |
| Published SDK correctness control | Pinned mixed Sapling/Orchard input-selection regression from [librustzcash#3133](https://github.com/zcash/librustzcash/issues/3133), classified as an availability/correctness defect rather than a discovered privacy leak |
| Zallet historical source witness | Pinned before/after `FullPrivacy` test and controls for [zallet#677](https://github.com/zcash/zallet/pull/677), preserved as separate original evidence |
| Real Zallet automatic policy discovery | Two separately compiled pinned `zallet-core` binaries, typed Proposal-derived policy observations, dedicated `zallet-privacy` generator, three-case control/difference campaign, minimized saved-case replay with SHA-linked provenance; [reproduce and review](zallet-auto-discovery.md) |
| Static site source | `site/` provides the landing page and read-only historical-evidence explorer; it does not execute wallet fuzzing in a browser |

The observed decision-difference engine is **generic** and the Orchard-only SDK restriction remains separate from `FullPrivacy`. The new dedicated Zallet historical adapter closes [#40](https://github.com/Ay-obami/ShieldSpec/issues/40)'s technical discovery gap: a verified local run compared three genuine policy observations from independently compiled Zallet revisions, detected the documented historical difference, minimized it, and replayed the same verdict direction. This is not a new upstream vulnerability, and no live wallet funds or network broadcasts were involved.

## Completed sprint issues and current integration gaps

The original implementation and release-presentation tickets [#35](https://github.com/Ay-obami/ShieldSpec/issues/35), [#36](https://github.com/Ay-obami/ShieldSpec/issues/36), [#37](https://github.com/Ay-obami/ShieldSpec/issues/37), and [#41](https://github.com/Ay-obami/ShieldSpec/issues/41) were audited and closed on October 28, 2026, with code and CI evidence linked in their closing comments. The onboarding [#39](https://github.com/Ay-obami/ShieldSpec/issues/39) and initial site [#38](https://github.com/Ay-obami/ShieldSpec/issues/38) are also closed. No currently open issue should be inferred from these historical links.

The remaining work is **new integration and release work**, not unfinished acceptance criteria of those tickets:

- **Independent adoption:** Have a wallet/SDK integrator execute the documented commands on a clean environment and provide actionable feedback.
- **New wallet-policy adapters:** Extend beyond the pinned Zallet historical `FullPrivacy` adapter and SDK Orchard-only spend restriction; do not synthesize policy verdicts from static snapshot findings.
- **Broader differential coverage:** Compare truly different compatible SDK/wallet builds and add replayable regression cases for verified differences.
- **Release readiness:** Review the post-`v0.1.0` API/protocol changes, crate packaging and consumer compatibility before publishing another tag; see [release-readiness audit](release-readiness.md).

The [developer website](https://ay-obami.github.io/ShieldSpec/) is already published. Its content and uptime remain ongoing maintenance rather than an uncompleted initial deployment.

## Planned after integration

- **More real wallet-policy adapters:** Expand beyond the implemented Zallet `FullPrivacy` historical probe and pinned SDK Orchard-only policy, without manufacturing verdicts from ShieldSpec warnings.
- **Maintained regression corpus:** Known expected cases and verified discoveries with stable source provenance, replay identities and clear semantics.
- **Versioned contracts:** Document migrations for scenario JSON, normalized snapshots, policy-observation envelopes, and differential reports.
- **Developer policy configuration:** Let wallets select and justify invariants, including legitimate cross-pool transfers, without overstating their meaning.
- **Contributor and release workflow:** Small fixture templates, review guidance, release artifacts, and compatibility matrices.
- **Website maturity:** Navigation/feedback channels and ongoing deploy checks, keeping the static explorer labeled as recorded data only.

## Out of scope

No mainnet signing or broadcast, custody of real keys, live browser-side wallet fuzzing, chain-analytics attribution, numeric anonymity score, guarantees of privacy, or reframing already reported upstream behavior as a novel security discovery.

For a hands-on start see [Getting Started](getting-started.md). For the evidence boundary see [Privacy Model](privacy-model.md) and [Wallet-Policy Observations](wallet-policy-observations.md).
