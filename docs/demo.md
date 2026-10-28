# Zecathon demo storyboard

For supporting narration and scene timing, see [the submission notes](submission.md) and [current browser-first script](demo-video-script.md). This technical storyboard uses runnable commands from merged `main`; it does not depend on a hosted video. The [developer website](https://ay-obami.github.io/ShieldSpec/) and its [hands-on guide](https://ay-obami.github.io/ShieldSpec/docs.html) explain the tool and display committed evidence, but the browser does not run a wallet or a live fuzzing service.

Keep three demonstrations distinct: (1) the **historical Zallet automatic rediscovery** runs independently compiled, pinned upstream `FullPrivacy` implementations through ShieldSpec's general fuzzer; (2) the **controlled injected fault** deterministically proves the generic detection, minimization and replay path; and (3) the **real SDK self-comparison** proves that the runner executes genuine Orchard-only wallet input selection. None is a newly discovered upstream vulnerability.

## The problem in one sentence

A wallet upgrade can keep producing valid Zcash transactions while silently changing their privacy properties.

## 30-second magic moment: an intentionally injected regression

From the repository root:

```bash
cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- demo --format human
```

The demo builds real proposal state through the pinned Zcash wallet test harness. The baseline has no configured privacy findings. The candidate then receives an intentionally injected **demo-only** change-pool regression *after* normalization. This is not a difference between two independently compiled SDK revisions.

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

## 60-second real SDK control: Orchard-only spend policy

Build the real-wallet runner and generic differential fuzzer from `main`:

```bash
cargo build --locked -p shieldspec-zcash-runner --bins

target/debug/shieldspec-fuzz fuzz \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --baseline-arg=--wallet-spend-policy --baseline-arg=orchard-only \
  --candidate-arg=--wallet-spend-policy --candidate-arg=orchard-only \
  --scenario-family ironwood --seed 2026 --cases 4 --format json
```

Expect `status: "no_regression_found"` and `baseline_wallet_policy_observations`, `candidate_wallet_policy_observations`, and `comparable_wallet_policy_cases` all equal to `4`. This is a **same-runner negative control**: it observes the real pinned SDK's `zcash_spend_orchard_only_v1` verdict on both sides, not a disagreement between independently built SDK versions.

Inspect the actual Sapling-funded rejection in ordinal 2:

```bash
target/debug/shieldspec-fuzz replay \
  --scenario-family ironwood --seed 2026 --ordinal 2 \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --baseline-arg=--wallet-spend-policy --baseline-arg=orchard-only \
  --candidate-arg=--wallet-spend-policy --candidate-arg=orchard-only \
  --format json
```

The recorded wallet verdicts agree on `rejected` because the restriction permits spending **Orchard inputs only**, while this wallet is funded from Sapling. Orchard-funded ordinals 0 and 1 instead report `accepted`. On rejection, the snapshot remains a real **unrestricted diagnostic proposal** showing what could have been funded; it is not presented as a policy-approved spend. At NU6.3 activation, `SS006` may flag the same real pool value-balance movement on both sides without creating a differential failure. Neither result is a test of Zallet's separate `FullPrivacy` policy.

## Show what the real-wallet runner covers

The runner tests execute SDK proposal machinery against isolated synthetic wallet chains. They include:

- Orchard- and Sapling-funded payments to shielded recipients, including actual Sapling recipient and mixed Unified Address receiver selection;
- ZIP 320 shielded-to-TEX proposals with a two-step ephemeral transparent bridge that is **not** mislabeled as wallet transparent funding or change;
- transparent-UTXO-funded shielded payments (opt-in generator), mixed Sapling/Orchard input selection, fragmented notes, ZIP 317 fees, shielded change, and accounting invariants;
- NU6.3 activation-boundary Orchard/Ironwood routes and the observable cross-pool value-balance rule `SS006`.

These are SDK-backed tests, not hand-written proposal JSON. They cover behavior and known edge cases; their existence does not assert an upstream privacy flaw. For the comprehensive runner suite:

```bash
cargo test --locked -p shieldspec-zcash-runner --all-targets
```

The optional transparent funding generator uses `--transparent-funding` with `fuzz` or seed/ordinal `replay`. It is separate from `--scenario-family ironwood`; do not combine them.

## Published motivation and historical evidence

The motivation is real wallet-development risk, but distinguish it from what each demo proves. [Zallet #693](https://github.com/zcash/zallet/issues/693) documents proposal-testing difficulty; [librustzcash #2614](https://github.com/zcash/librustzcash/issues/2614) concerns independently reproduced transparent send-max **correctness** behavior, not an established privacy leak; [librustzcash #2775](https://github.com/zcash/librustzcash/issues/2775) illustrates demand for explicit metadata leak-regression assertions. These are contextual Zcash engineering examples, not claimed fixes delivered by ShieldSpec.

### Actual automated discovery of the already-fixed Zallet policy difference

The standalone [historical Zallet #677](../evidence/historical-zallet-677/README.md) before/after witness runs the upstream `FullPrivacy` policy around its documented fix. **In addition**, ShieldSpec's general differential fuzzer now executes independently compiled, pinned affected and corrected Zallet policy runners. Two control scenarios agree on both versions; the historical Ironwood→Sapling change scenario differs, is semantically minimized, and replays with executable provenance verified. The validator requires the real policy difference, not a fabricated post-processing delta.

For an executable build-to-replay walkthrough, run the commands in [automated Zallet discovery](zallet-auto-discovery.md). The source builds can be lengthy, so a short browser-first video should instead show the [successful historical discovery CI run](https://github.com/Ay-obami/ShieldSpec/actions/runs/40616469538), its pinned revisions, and the website's evidence tabs; it must explain that the bug was **previously disclosed and fixed upstream**. This is the strongest proof of genuine historical independent-runner differential detection, not a new disclosure.

Say:

> ShieldSpec makes wallet proposal behavior inspectable: execute real SDK code on deterministic wallet states, compare explicit privacy rules and genuinely observed policy decisions, and turn differences into smaller, replayable developer tests. Its general fuzzer has automatically reproduced Zallet's *already-fixed* historical policy difference. The separate fault-injection demonstration is labeled, and the SDK self-comparison is a control.

Do **not** claim the injected transparent-change case or same-runner Orchard-only control discovered a new upstream privacy bypass, and do not claim a clean campaign proves anonymity.

## CI close

Show the `Privacy differential fuzzer smoke` job. It verifies:

- a deterministic baseline-vs-itself campaign produces no differential regression;
- four real SDK NU6.3 Orchard-only policy observations are recorded on both sides, including accepted Orchard and rejected Sapling funding;
- the injected judge demo exits `2` and includes `SS003_TRANSPARENT_CHANGE`;
- the exact differential replay also exits `2`;
- the committed known-behavior TEX corpus case replays its recorded rule without false transparent-funding/change findings.

Then show the original ShieldSpec Action as the lightweight **snapshot-only** CI gate. The real-wallet tests and fuzzer smoke job run in repository CI; the Action by itself does not execute the wallet SDK or compare SDK-policy verdicts.

## Precise closing claim

Use:

> ShieldSpec turns proposal-level privacy changes into reproducible developer tests before they ship.

Do not use:

> ShieldSpec proves this transaction is anonymous.

A clean result only means no configured regression was found in the supplied or generated proposal facts that were exercised.
