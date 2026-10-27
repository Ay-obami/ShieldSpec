# ShieldSpec — ready-to-film 2:30 technical demo

**Presentation:** 1920×1080, terminal-first, readable fonts and a voiceover. The actual command output and historic logs must be shown; no simulated discovered vulnerability. A separate [automatically generated captioned video](media/shieldspec-zecathon-captioned-demo.mp4) already exists (about 2:20; no voiceover). This 2:30 storyboard is the optional stronger narrated/live-terminal recording.

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
- Review audio for accuracy; upload the video to a public link and attach it to Zecathon. A repository alone does not mean a hackathon submission was sent.
