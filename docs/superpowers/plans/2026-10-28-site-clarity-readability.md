# ShieldSpec Website Clarity and Readability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make ShieldSpec understandable and usable to first-time developers without reverting to the crowded original style.

**Architecture:** Extend the existing static homepage, add `site/docs.html` using the shared stylesheet/JS copy system, and update tests and site README. Preserve the historical evidence JSON and runners unchanged.

**Tech Stack:** HTML, CSS, vanilla JS, Node `site/tests/site.test.mjs`, GitHub Pages.

**Spec:** `docs/superpowers/specs/2026-10-28-site-clarity-readability-design.md`.

## Global Constraints

- Flat off-white `#f8f9fb`, charcoal `#15171c`, ZEC gold `#c49336`, no gradients or green/violet.
- Every technical claim must be checked against actual repository docs and pinned tests; no invented vulnerabilities, releases or deployments.
- Preserve existing evidence tabs, copy controls, links, semantic HTML, and reduced-motion handling.
- No local signing, wallet secrets, funded wallets, or real network transactions.

## Review Focus

- Navigation: home and docs routes both resolve, including copy buttons and return links.
- Reader: introductory text answers what goes in, what gets tested, and what a result provides.
- CLI: snapshot and runner commands use existing options and accurately label expected nonzero statuses.
- Browser: small text stays readable; long commands scroll within cards and do not widen the page.
- Evidence: historical Zallet proof remains verifiable and distinguished from opt-in Orchard-only SDK control.

---

### Task 1: Navigation and developer guide

**Files:** `site/tests/site.test.mjs`, `site/docs.html`, `site/index.html`, `site/README.md`.

- [ ] Write a failing navigation/instruction integration test for a real Docs route, prereqs, runnable snapshot CLI, runner control, saved replay, CI, statuses, and copy targets.
- [ ] Verify expected failure with `node site/tests/site.test.mjs`.
- [ ] Implement `docs.html` with shared app assets, structured instructions and linked resources; update home Docs navigation and full-guide links.
- [ ] Verify green tests and document local preview/guide in site README.

### Task 2: Explanatory homepage and readable type

**Files:** `site/tests/site.test.mjs`, `site/index.html`, `site/styles.css`.

- [ ] Add failing regression checks for discoverable full explanation, common privacy observations, actual output/provenance, larger reading text and mobile readability.
- [ ] Verify failure, then implement text hierarchy and comfortable visual spacing with enlarged text and darker supporting copy.
- [ ] Keep real proof compact, retain interactive controls and no gradients; verify all site tests.

### Task 3: Browser QA and release

**Files:** website and tests as needed.

- [ ] Run `node site/tests/site.test.mjs`, `node --test site/tests/site.test.mjs`, `node --check site/app.js`, and `git diff --check`.
- [ ] Inspect live browser screenshots at the top and evidence/docs positions, use real interactions, fix any observed issues.
- [ ] Publish via reviewed PR to main and verify GitHub Pages serves fresh versioned assets and the guide.
