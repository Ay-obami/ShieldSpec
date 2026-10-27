# ShieldSpec product roadmap

ShieldSpec is **reusable privacy-regression infrastructure for Zcash wallet developers**, not a one-off vulnerability claim. This page distinguishes the stable snapshot gate, features implemented in the open development PR, and work that still needs real wallet integration. The [public issue tracker](https://github.com/Ay-obami/ShieldSpec/issues) and linked PRs hold review details; "implemented" here does not imply every feature has been released to `main`.

## Available on main

| Capability | Boundary |
| --- | --- |
| Privacy policy core and normalized proposal model | Explicit proposal-level transparent exposure, routing and related invariants |
| Snapshot CLI | `check`, `diff`, human/JSON/SARIF and 0/1/2 exit status |
| GitHub Action (`v0.1.0`) | Baseline-versus-candidate snapshot privacy check; does **not** spin up a wallet |
| Deterministic real proposal harness and controlled demo | Isolated synthetic chain and wallet state; real Zcash SDK proposal construction, baseline/candidate processes, replay and deliberate fault injection |

The independently published [GitHub Pages site](https://ay-obami.github.io/ShieldSpec/) returned **HTTP 200 on 2026-10-27**, but the local `main` Git tree does **not yet contain** `site/` or the Zallet historical evidence files. Their source is available in the current feature branch. Public URL reachability and a merge to `main` are separate facts.

None of these tools proves wallet anonymity. Historical upstream behavior must not be described as a ShieldSpec-discovered zero-day.

## Implemented in the current feature branch — pending merge

[PR #42 — generic wallet-policy difference and Ironwood scenarios](https://github.com/Ay-obami/ShieldSpec/pull/42) remains **open/unmerged as of 2026-10-27**. These capabilities can be run on that PR head but should not be described as released on `main`:

| Capability | Verified scope and limits |
| --- | --- |
| `SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE` | Detects aggregate shielded-pool public value balance crossings in real normalized proposals; **not** proof of a wallet-policy violation |
| First-class policy-verdict runner protocol | Optional named `accepted`/`rejected` observation alongside the full proposal snapshot, preserving operational errors separately |
| Generic verdict differential | Comparable policy names, decision differences, minimization, provenance recording, saved-case replay and coverage counts |
| Real SDK spend-policy observation | Opt-in `--wallet-spend-policy orchard-only` invokes actual pinned SDK Orchard-only source restriction; it is **not Zallet FullPrivacy** |
| Activation-aware scenario family | `--scenario-family ironwood --cases 4` covers Orchard/Sapling funding before/at NU6.3, including real Orchard→Ironwood and Sapling→Ironwood proposals |
| Provenance and replay safeguards | Runner SHA-256 and exact args verified on saved replay; SDK labels are user-declared, not hash-to-source proof |
| Zallet historical source witness | Pinned before/after `FullPrivacy` test and controls for [zallet#677](https://github.com/zcash/zallet/pull/677), independent of generic fuzzing; source/evidence not yet on `main` |
| Static site source | `site/` includes the developer landing and read-only explorer; public Pages is live, but the source directory is not yet in local `main` |

The observed decision-difference engine is **generic**: tests prove detection, shrinking, and replay with controlled policy changes. The real Orchard-only adapter proves an actual SDK decision is obtainable. These two facts must not be combined into a claim of automatically discovering Zallet's historical bypass.

## In progress / integration gaps

- [#40 — automatic discovery and replay integration](https://github.com/Ay-obami/ShieldSpec/issues/40): a general campaign that independently builds affected/corrected wallet runners, runs the **actual Zallet FullPrivacy policy**, discovers its historical verdict difference, and saves/replays the same minimized finding is **not yet automatic**. The pinned [historical witness](../evidence/historical-zallet-677/README.md) remains separate.
- [#36 — wallet policy input and provenance](https://github.com/Ay-obami/ShieldSpec/issues/36) and [#37 — cross-pool oracle](https://github.com/Ay-obami/ShieldSpec/issues/37): the generic protocol, real SDK Orchard-only mode, cross-pool rule and coverage reporting exist on PR #42; additional wallet adapters and review/merge/release remain.
- [#35 — realistic cross-pool scenarios](https://github.com/Ay-obami/ShieldSpec/issues/35): the four activation-boundary cases are implemented on PR #42. Extend beyond this deliberately small matrix after baseline coverage is established.
- [#39 — onboarding/docs](https://github.com/Ay-obami/ShieldSpec/issues/39): maintain copy-paste commands for `main` versus the feature preview, explicit exit semantics, CI example, and limits.
- [#41 — sprint integration](https://github.com/Ay-obami/ShieldSpec/issues/41): review compatibility, verify tests and merge order, then document a release/tag only after it exists.

The [#38 developer-first website](https://github.com/Ay-obami/ShieldSpec/issues/38) is publicly reachable; future content or uptime work should not be confused with initial deployment verification.

## Planned after integration

- **More real wallet-policy adapters:** A concrete Zallet `FullPrivacy` runner and distinct policy versions, without manufacturing verdicts from ShieldSpec warnings.
- **Maintained regression corpus:** Known expected cases and verified discoveries with stable source provenance, replay identities and clear semantics.
- **Versioned contracts:** Document migrations for scenario JSON, normalized snapshots, policy-observation envelopes, and differential reports.
- **Developer policy configuration:** Let wallets select and justify invariants, including legitimate cross-pool transfers, without overstating their meaning.
- **Contributor and release workflow:** Small fixture templates, review guidance, release artifacts, and compatibility matrices.
- **Website maturity:** Navigation/feedback channels and ongoing deploy checks, keeping the static explorer labeled as recorded data only.

## Out of scope

No mainnet signing or broadcast, custody of real keys, live browser-side wallet fuzzing, chain-analytics attribution, numeric anonymity score, guarantees of privacy, or reframing already reported upstream behavior as a novel security discovery.

For a hands-on start see [Getting Started](getting-started.md). For the evidence boundary see [Privacy Model](privacy-model.md) and [Wallet-Policy Observations](wallet-policy-observations.md).