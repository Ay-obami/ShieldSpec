# ShieldSpec product roadmap

ShieldSpec is intended to become **reusable open-source privacy regression infrastructure** for Zcash wallet maintainers. Zecathon is a delivery milestone, not the end of the project.

The roadmap separates verified features from active and speculative work. The public [GitHub issues](https://github.com/Ay-obami/ShieldSpec/issues) are the source of truth for engineering progress.

## Available — currently implemented

| Capability | Current boundary |
| --- | --- |
| Rust privacy policy core | Assesses normalized Zcash proposal snapshots and explicit transparent exposure/routing rules |
| Snapshot CLI | `check`, `diff`, human/JSON/SARIF output and stable 0/1/2 status contract |
| GitHub Action | A lightweight baseline-vs-candidate privacy gate for snapshot files |
| Real proposal runner | Isolated in-memory wallet states and real Zcash proposal construction for supported Orchard and ZIP-320 TEX scenarios |
| Differential fuzz loop | Deterministic scenario generation, two runner processes, assessment, semantic minimization and replay |
| Reproduction identities | Runner executable SHA-256, declared SDK labels, seeds, ordinals and minimized corpus artifacts |
| Historical source evidence | Pinned before/after Zallet wallet-core privacy-policy witness for [zallet#677](https://github.com/zcash/zallet/pull/677), with control cases; separate librustzcash correctness history |
| Website source | Developer-first static product landing and read-only historical evidence explorer, under `site/`. **Public hosting is not yet confirmed.** |

A passing run asserts only that specified invariants held for scenarios executed. It does not guarantee that users are anonymous or that a wallet has no other leak.

## In progress — current product sprint

**Core & Tooling upgrades:**

1. [#35 — realistic cross-pool scenarios](https://github.com/Ay-obami/ShieldSpec/issues/35): meaningful Ironwood/Sapling pool combinations with semantically valid values.
2. [#36 — authentic policy runner input and provenance](https://github.com/Ay-obami/ShieldSpec/issues/36): reusable, machine-readable real policy outcomes with strict operational error distinctions.
3. [#37 — cross-pool privacy oracle](https://github.com/Ay-obami/ShieldSpec/issues/37): explicit leaked-value-balance semantics and differential verdict observations.
4. [#40 — automatic discovery and replay integration](https://github.com/Ay-obami/ShieldSpec/issues/40): ordinary general campaign finds a real historical divergence without a purpose-built comparison flag and stores a minimized replay.

The historical Ironwood→Sapling FullPrivacy regression is **not yet automatic** in the general fuzzer. Existing evidence is an independent previously published and fixed upstream reproduction, not a novel vulnerability.

**Product upgrades:**

5. [#38 — developer-first website](https://github.com/Ay-obami/ShieldSpec/issues/38): source integrated into the feature branch, with static tests; hosting remains.
6. [#39 — onboarding and docs](https://github.com/Ay-obami/ShieldSpec/issues/39): installation, CI examples, responsible claim boundaries and contributor documentation.
7. [#41 — final sprint integration](https://github.com/Ay-obami/ShieldSpec/issues/41): compatibility, full verification, releases, deployment status and evidence-grade demo.

These are goals, not promises of completion or timing. Each issue needs verified checks before being marked done.

## Planned — after the sprint

- **Broader wallet adapters:** Make the real proposal/policy runner interface easy to integrate with additional Zcash wallet implementations without copying our harness.
- **Maintained regression corpus:** Curate minimized cases with explicit expected semantics, source versions, checksums, and stable CI replay.
- **Stable developer interfaces:** Version the scenario, policy-observation, normalized snapshot, and output report schemas with migration notes.
- **Better rule configuration:** Allow wallets to select relevant invariants, with explicit distinction between acceptable cross-pool behavior and policy violations.
- **Contributor workflow:** Small fixture guide, test templates, code review guidelines, issue taxonomy and reproducible release artifacts.
- **Website deployment and developer feedback:** Publish to a verified domain, add documentation navigation and issue feedback only after a deployment can be maintained responsibly.

## Out of scope for now

- Broadcasting real Zcash transactions or handling mainnet wallet keys.
- Browser-side live wallet fuzzing presented as a backend without one.
- Pretending a previously documented upstream issue is a new bug bounty discovery.
- Claiming a statistical anonymity score or end-to-end privacy guarantee from a proposal snapshot alone.

For a first hands-on experience, use [Getting Started](getting-started.md). For privacy-model detail, see [the privacy model](privacy-model.md).
