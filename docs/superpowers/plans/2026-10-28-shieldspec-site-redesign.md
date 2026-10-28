# ShieldSpec Site Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the approved spacious gold/charcoal ShieldSpec landing page with truthful automated historical Zallet evidence.

**Architecture:** Retain static `site/index.html` + `site/styles.css` + `site/app.js` + curated `site/data/cases.json`. Keep the interactive explorer and controls; redesign the page with flat surfaces, clean hierarchy, responsive typography and reduced-motion-aware micro-interactions.

**Tech Stack:** HTML5, CSS, dependency-free browser JavaScript, `node --test`, GitHub Pages workflow.

**Spec:** `docs/superpowers/specs/2026-10-28-shieldspec-site-redesign-design.md`.

## Global Constraints

- Off-white `#F8F9FB`, white, charcoal `#15171C`, near-black `#17191F`, gold `#C49336`; no gradients, green or purple.
- Responsive; skip links, keyboard tabs, copy status, reduced motion, no runtime dependencies.
- Public claims must match merged PR #45 and successful GitHub Actions run 40616469538. Historical upstream issue fixed; not a new vulnerability.

## Review Focus

- No CSS gradients or legacy green styling remain: static test `site/tests/site.test.mjs`.
- Narrow mobile screen: nav/hero/result and comparison cards must not overflow or truncate content.
- Unavailable `cases.json`: static historical result still visible and notice accurate.
- Clipboard permission denied: command stays keyboard selectable, status remains accessible.
- Historical proof vs browser demo: external links and copy distinguish live runner result from static explainer.

---

### Task 1: Lock visual and evidence contract with tests

**Files:** Modify `site/tests/site.test.mjs`.

- [ ] Add visual contract asserting gold/charcoal CSS, no `gradient(`, and no green/purple CSS tokens.
- [ ] Update historical test to assert `discoveredByShieldSpec: true`, `newVulnerability: false`, exact pinned revisions, provenance linked to PR #45.
- [ ] Assert hero's verified result, shorter copy, and a visible concise roadmap complete status.
- [ ] Run `node --test site/tests/site.test.mjs` and observe new assertion failures.

### Task 2: Update source data and faithful product copy

**Files:** Modify `site/index.html`, `site/data/cases.json`, `site/app.js`, `site/README.md`.

- [ ] Make the historical case data and static fallback describe real generalized fuzzer discovery, minimized and verified replay, no new vulnerability.
- [ ] Replace the injected-fault terminal hero with verified Zallet historical comparison and evidence links; preserve a separate intentionally-injected demo command in quickstart.
- [ ] Remove stale “not automatically rediscovered” claims in FAQ/roadmap/explorer/README.
- [ ] Adapt panel provenance to differentiate primary regression and unchanged controls; retain tab keyboard behavior.
- [ ] Run site tests to verify content assertions pass.

### Task 3: Redesign flat responsive site

**Files:** Replace `site/styles.css`, adjust `site/index.html` classes as needed.

- [ ] Build light neutral navigation/hero/sections, dark compact evidence surfaces, restrained gold CTA/links, no gradients.
- [ ] Space hero so detailed cards appear below fold, retain full sections with improved readable hierarchy.
- [ ] Make responsive widths and keyboard/reduced motion safe; check narrow layout and contents of all sections.
- [ ] Run `node --test site/tests/site.test.mjs`, `node --check site/app.js` and static style grep.

### Task 4: Verify and publish

**Files:** Site files and spec/plan only.

- [ ] Run full site tests and inspect browser screenshot at desktop/mobile width.
- [ ] Review `git diff --check`, unchanged source outside scope, and source links.
- [ ] Commit/push a focused branch, open PR against `main`, wait for required checks and merge when green.
- [ ] Verify GitHub Pages deployed content; report live URL, actual checks and any limitations.
