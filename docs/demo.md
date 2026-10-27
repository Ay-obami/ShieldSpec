# Zecathon demo storyboard

## The problem in one sentence

A wallet upgrade can keep producing valid Zcash transactions while silently changing their privacy properties.

## 30-second magic moment

From the repository root:

```bash
cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- demo --format human
```

The demo builds real proposal state through the pinned Zcash wallet test harness. The baseline has no configured privacy findings. The candidate then receives an intentionally injected **demo-only** change-pool regression.

Expected headline output:

```text
ShieldSpec Privacy Differential Fuzzer
status: privacy_regression

baseline findings: 0
candidate findings: 2
  SS003_TRANSPARENT_CHANGE
  SS005_PRIVACY_BOUNDARY

new/worsened privacy regressions: 2
...
reproduce: ... replay --seed 148 --ordinal 0 ... --demo-regression transparent-change
```

The process exits with status `2`.

Say this explicitly during the demo:

> The transaction proposal itself comes from real librustzcash wallet machinery. The transparent-change regression in this judge demo is deliberately injected after proposal normalization so the entire detection, minimization, and replay pipeline is deterministic. We do not claim this injected fault is a librustzcash vulnerability.

## What happened before the finding

The runner:

1. created an isolated in-memory Zcash wallet;
2. generated and scanned synthetic compact blocks containing shielded wallet funds;
3. exercised real input selection, fees, change, pool choice, and proposal construction;
4. normalized the resulting real `Proposal`;
5. ran the identical scenario against baseline and candidate runner processes; and
6. compared privacy findings rather than treating every structural difference as a bug.

## Show the shrinker

Point to the reported minimal scenario. The failing generated state is repeatedly simplified while still executing the real proposal path and preserving the differential privacy finding.

This is the useful loop:

```text
generated wallet state
      |
real Zcash proposal
      |
baseline vs candidate
      |
privacy regression
      |
semantic shrink
      |
minimal replay case
      |
permanent regression test
```

## Prove replay

Run the reproduction command printed by the demo:

```bash
cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- \
  replay \
  --seed 148 \
  --ordinal 0 \
  --policy standard \
  --demo-regression transparent-change \
  --format human
```

It should again exit `2` and report the same new/worsened privacy rules.

## Show a real multi-step Zcash path

The runner test suite also exercises a genuine ZIP 320 shielded-to-TEX proposal. librustzcash constructs two proposal steps with an ephemeral transparent bridge. ShieldSpec preserves the prior-step origin so that internal bridge value is not mislabeled as ordinary wallet transparent funding or transparent change.

This is an important false-positive check: the tool must understand intended Zcash behavior before it can credibly call something a regression.

## Published motivation

Keep this brief in the video. Show [zcash/zallet#693](https://github.com/zcash/zallet/issues/693) as the primary evidence that realistic proposal construction can block direct privacy-policy testing. Optionally flash [zcash/librustzcash#2614](https://github.com/zcash/librustzcash/issues/2614) and [zcash/librustzcash#2775](https://github.com/zcash/librustzcash/issues/2775) as supporting evidence that proposal edge cases can escape hand-written tests and that privacy properties need explicit leak-regression assertions.

Do **not** say ShieldSpec closes any of those issues. Say:

> Zcash developers have publicly documented proposal testability gaps and the need for explicit privacy regression assertions. ShieldSpec turns that testing problem into an automated workflow: generate wallet state, invoke the real proposal engine, compare privacy invariants, and minimize failures into deterministic regression cases.

Then immediately return to the live ShieldSpec demo; the upstream issues validate the idea but are not the center of the project.

## CI close

Show the `Privacy differential fuzzer smoke` job. It verifies:

- a deterministic baseline-vs-itself campaign produces no regression;
- the judge demo exits `2` and includes `SS003_TRANSPARENT_CHANGE`;
- the exact differential replay also exits `2`;
- the committed known-behavior TEX corpus case replays its recorded rule without false transparent-funding/change findings.

Then show the original ShieldSpec Action as the lightweight production CI gate.

## Precise closing claim

Use:

> ShieldSpec turns proposal-level privacy changes into reproducible developer tests before they ship.

Do not use:

> ShieldSpec proves this transaction is anonymous.

A clean result only means no configured regression was found in the supplied or generated proposal facts that were exercised.
