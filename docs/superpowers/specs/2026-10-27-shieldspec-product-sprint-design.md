# ShieldSpec Product Sprint — Approved Direction

Date: 2026-10-27
Product: ShieldSpec (Zcash privacy differential fuzzing and regression testing)
Audience: Zcash wallet and SDK developers first; accessible to judges, maintainers, recruiters, and sponsors.
Implementation location: authorized Ubuntu laptop, via Remote Desktop Commander only.
Repository: Ay-obami/ShieldSpec. Preserve existing privacy regression CLI, GitHub Action, fixtures, historical evidence, and previous submissions.

## Intent
Make ShieldSpec a credible, reusable open-source developer tool with a public website and a general fuzzing pipeline that can independently rediscover a documented historical Zallet cross-pool privacy-policy failure. The Zecathon submission is one milestone, not the product's organizing principle.

## Two independent deliverables
1. **Core upgrade (A+B)**: strengthen the privacy oracle around real cross-pool public value-balance exposure and integrate pinned, independently compiled Zallet policy runners into the general deterministic generator → compare → shrink → replay loop.
2. **Product upgrade**: a responsive, accessible, developer-first landing page that explains ShieldSpec, shows verified evidence transparently, and directs users to reproducible quickstart, library/CLI/GitHub Action examples, technical documentation, source, and release roadmap.

## Behavior and truthfulness
- Use actual Zcash/Zallet privacy-policy machinery. Never manufacture a runner verdict or relabel a controlled demo fault as a discovered upstream bug.
- Explicitly distinguish shielded pool transitions involving publicly revealed value balances from benign shielded routing; account for fees and privacy-policy exemptions. Test the policy criterion, not merely differing output pool names.
- Preserve known expected ZIP-320 ephemeral transparent bridge semantics. Include same-pool, cross-pool, and permissive-policy controls to prevent false positives.
- Compare **corrected** Zallet policy as baseline against **affected historical** Zallet as candidate when demonstrating a historical regression. Explain reverse chronology in the finding.
- Policy decisions must become first-class runner observations, distinct from ProposalSnapshot and operational/compile errors. If a runner lacks policy support, return a compatible explicit capability indication instead of guessing.
- Fuzz generation should include valid Ironwood→Sapling cross-pool policy scenarios; replay identity must include exact upstream commits, runner binaries/hashes, deterministic seed/ordinal, original and minimized scenario, and observed verdicts.
- Historical upstream zallet#677 issue is a documented previously fixed bug; independent rediscovery by ShieldSpec is not a novel vulnerability disclosure.
- No live wallet RPC, mainnet funds, signing, broadcast, or browser-executed Rust fuzz campaign in this sprint.

## Website architecture
- Prefer a standalone static, buildless web frontend in `site/` within the existing GitHub repository; no new backend or credentials.
- Product hero: “Catch privacy regressions before they reach production.”
- Sections: concrete failure/evidence, product mechanics, differential and replay workflow, demo/report explorer using checked-in verified artifacts, quickstart integration, technical boundaries, open-source roadmap, FAQ and persistent links to GitHub/docs.
- Real interactive report viewer may select among committed verified finding/control cases; it MUST say these are historical/replay artifacts, not freshly run fuzzing jobs.
- Use a restrained premium security/developer visual design: dark neutral backgrounds, emerald accents, readable mono output, strong contrast, accessible keyboard focus, mobile-first/responsive.
- Provide social meta, semantic navigation, accessible labels, no misleading vanity adoption metrics. Use local assets and relative paths.
- Hosting is separate: site source and build are required this sprint; production deployment needs a verified destination/domain and available access. Do not pretend local preview is public hosting.

## Options considered
- A: oracle only — fast, but historical policy regression remains invisible when snapshots match.
- B: pinned real policy runners + verdict protocol — strong fidelity; requires careful compatibility.
- C: complete historical wallet RPC — too large, costly and unnecessary for a regression tool this sprint.
- Chosen A+B, alongside the product website. No live backend.
- Site alternatives: a hackathon marketing microsite (reject: ephemeral) or full SaaS dashboard (defer: unneeded infrastructure); choose documentation-led developer-product site.

## Quality, preservation and acceptance
- All work executed on laptop via Remote Commander. Do not edit project using an internal container.
- GitHub issues are the sprint's unit of work. Each parallel worker gets an isolated Git worktree/branch and non-overlapping ownership; integration agent coordinates interfaces and merges.
- TDD: new behavior is specified by a failing test before implementation. Verify full Rust and site suites, README quickstart, deterministic replay, and historical findings.
- CI runs no silent network-heavy historical builds on every PR by default; reproducibility job documented and optionally manual/appropriate matrix.
- Existing functionality and historical evidence remain unchanged unless compatibility tests justify a narrow migration.
- Deliver actual issue URLs, test outcomes, commits/PR, honest limitations, website preview/deployment status.
- Do not claim upstream bug discovery or a new privacy vulnerability without independent evidence.
