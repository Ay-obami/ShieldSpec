# ShieldSpec — archived walkthrough and technical demo scripts

These are the 2026 Zecathon recording storyboards, retained for historical reference. The final video was uploaded and the submission completed; this document is **not an outstanding recording checklist**. For current installation and reproducible evidence, use [getting started](getting-started.md) and [Zallet auto-discovery](zallet-auto-discovery.md). No MP4 is tracked in the repository.

## Archived recording plan — browser-first website walkthrough

Record **actual continuous website interaction**, not a slideshow, from the [live website](https://ay-obami.github.io/ShieldSpec/) and [new developer guide](https://ay-obami.github.io/ShieldSpec/docs.html), at 1600×900 or higher. Use a natural **female** English narration, captions, legible code and deliberate scrolling/clicking. The approved narration is approximately 96 seconds; aim for a 1:45–2:00 finished video, with a hard ceiling of **2:30**. Keep the no-gradients charcoal, off-white and ZEC-gold website visible; no full-screen static title cards that conceal the product.

Earlier browser-first and captioned technical recordings predated expanded coverage and the website redesign. The timing and narration below describe the historical production plan rather than a currently requested recording.

The website is a static evidence viewer and documentation site, **not** an in-browser fuzzer. The strongest real demonstration now exists: ShieldSpec's general engine **automatically rediscovered the previously documented, already-fixed Zallet #677 privacy-policy difference** using two independently compiled, pinned upstream policy runners, then minimized and replayed it. That claim is different from the labeled fault-injection demo and SDK same-runner negative control. Do not claim a new upstream vulnerability.

| Target time | Real browser interaction | Point to demonstrate |
| --- | --- | --- |
| 0:00–0:13 | Start on landing-page hero; move cursor over the two pinned historical verdicts | What goes wrong when a valid transaction exposes unexpected information |
| 0:13–0:28 | Scroll through “The problem,” the check coverage, and Generate → Execute → Compare → Replay | Concrete risks and the baseline-versus-candidate method |
| 0:28–0:54 | Interact with the historical evidence tabs, affected/corrected verdicts, upstream fix and saved replay proof | Actual automatic discovery of an old, fixed Zallet `FullPrivacy` difference, not an injected bug |
| 0:54–1:12 | Scroll to SDK coverage; enter the developer guide and show snapshot commands and real SDK examples | A developer can run actual tests and understand the supported scope |
| 1:12–1:28 | Scroll through historical reproduction, CI, and linked upstream Zcash issues #693, #2614 and #2775 | Auditable proof and authentic Zcash engineering motivation |
| 1:28–1:40 | Return to the home page; show the GitHub repository, documentation call to action and final result | Close on repeatable developer tooling |

### Voiceover draft (natural, measured delivery)

> A Zcash payment can be valid and still reveal more than its sender intended. ShieldSpec tests wallet transaction proposals before signing, so developers can catch privacy regressions during development.
>
> It generates deterministic wallet states, executes baseline and candidate implementations, and compares both observable outputs and named policy decisions. When they disagree, ShieldSpec reduces it to a replayable test with binary hashes.
>
> Here's the proof. We compiled two pinned Zallet versions around upstream fix six seventy-seven. The affected FullPrivacy policy accepted an Ironwood input with Sapling change, exposing a cross-pool balance. The corrected version rejected it. ShieldSpec's general fuzzer automatically detected, minimized and replayed that genuine historical difference. It was already disclosed and fixed. This isn't a new vulnerability.
>
> Real Zcash SDK scenarios also cover Orchard and Sapling funding, Unified Address receiver choice, shielded change, transparent shielding, fees and activation boundaries. The developer guide includes commands for snapshot checks, SDK runs, historical reproduction and CI.
>
> The broader need is documented in Zallet issue six ninety-three and librustzcash issues twenty-six fourteen and twenty-seven seventy-five: wallet edge cases are hard to test, and correctness alone doesn't establish privacy.
>
> ShieldSpec makes privacy behavior observable, differences reproducible, and regressions testable before they reach users.

Keep narration synchronized to **visible live interaction**. If the generated voice is slower than the time grid, adjust capture timing or trim pauses without cutting substantive evidence; never exceed 2:30. Verify final captions against domain spellings: ShieldSpec, Zcash, Zallet, Ironwood, Sapling, Orchard, FullPrivacy, ZIP-317, and librustzcash.

## Alternative archived recording plan — 2:30 terminal-first walkthrough

**Presentation:** 1920×1080, terminal-first, readable fonts and voiceover. Show actual command output and historical logs, with no simulated upstream discovery. A captioned technical recording was previously generated but is no longer tracked. The table below is an **alternative technical storyboard**, not the sequence of the completed browser-first film.

| Time | Screen | Spoken narration |
| --- | --- | --- |
| 0:00–0:15 | ShieldSpec title and architecture | “Wallet upgrades can change privacy properties even when their transactions still work. ShieldSpec turns those properties into regression tests.” |
| 0:15–0:37 | `evidence/historical-zallet-677/results/verdict.json`, the before/after accepted/rejected log lines, and upstream zallet#677 | “This is a real, documented wallet privacy-policy difference. An Ironwood input funding Sapling change was incorrectly accepted by `FullPrivacy` before the fix, but rejected afterward. We reproduced it with identical Rust tests against two actual Zallet revisions; this is not a newly discovered vulnerability.” |
| 0:37–1:24 | Run `target/debug/shieldspec-fuzz demo --format human`; focus on the baseline, candidate and minimized case | “ShieldSpec builds real Zcash proposals. To make the judge demo deterministic, we inject one transparent-change fault *after* proposal normalization. The tool detects two new privacy findings and shrinks the scenario.” |
| 1:24–1:48 | Run `target/debug/shieldspec-fuzz replay --seed 148 --ordinal 0 --policy standard --demo-regression transparent-change --format human` | “The seed and ordinal replay the same finding. Saved discoveries also identify the exact runner binaries by SHA-256 and reject silent build changes.” |
| 1:48–2:09 | Run the real ZIP-320 test and show ephemeral bridge output | “ShieldSpec understands the legitimate ZIP-320 ephemeral transparent bridge, so it doesn't misidentify an expected protocol operation as wallet transparent change.” |
| 2:09–2:23 | GitHub Actions CI and historical manual workflow | “The real proposal tests, differential smoke, replay, and authentic historical reproduction are all independently testable.” |
| 2:23–2:30 | Repo URL, one-sentence closing | “ShieldSpec turns proposal-level privacy changes into reproducible developer tests before they ship.” |

## Actual commands to record

From a Rust 1.95 checkout, after `cargo build --locked -p shieldspec-zcash-runner --bins`:

```bash
python3 -m json.tool evidence/historical-zallet-677/results/verdict.json
rg -n 'SHIELDSPEC_IRONWOOD_TO_SAPLING_FULL_PRIVACY|test result:' evidence/historical-zallet-677/results/before.log evidence/historical-zallet-677/results/after.log

target/debug/shieldspec-fuzz demo --format human
target/debug/shieldspec-fuzz replay --seed 148 --ordinal 0 \
  --policy standard --demo-regression transparent-change --format human

cargo test --locked -p shieldspec-zcash-runner \
  tex_scenario_uses_real_ephemeral_two_step_proposal -- --nocapture
```

The demo and differential replay **intentionally exit with code 2** as the expected policy failure. Never present that exit as a crash. Historical upstream reproduction is already recorded and can be verified through the separate GitHub Actions workflow; don't wait for a full recompilation during the short demo.

## Zcash-only references

- Repository: https://github.com/Ay-obami/ShieldSpec
- Real historical privacy issue/fix: https://github.com/zcash/zallet/pull/677
- Independent historical correctness issue/fix: https://github.com/zcash/librustzcash/issues/2614 and https://github.com/zcash/librustzcash/pull/2615
- Supporting testability issue: https://github.com/zcash/zallet/issues/693
- Supporting privacy-leak-testing issue: https://github.com/zcash/librustzcash/issues/2775

## Filming quality checks

- Text at least 20–24 px in 1080p recording and enough time to read each output.
- Real output, including the exact Git SHAs, seed, minimized input and two rule IDs.
- State the difference between the real **historical and fixed Zallet privacy-policy** proof, the independent `librustzcash#2614` correctness proof, and the **injected** ShieldSpec CLI privacy-fuzzer demonstration.
- Show one actual passing GitHub Actions run and the repository URL.
- For any future recording, review audio for accuracy and preserve the distinction between real historical evidence and injected test faults.
