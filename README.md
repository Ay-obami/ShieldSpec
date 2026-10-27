# ShieldSpec

ShieldSpec is privacy differential fuzzing and regression testing for Zcash wallet transaction proposals. It generates realistic wallet states, exercises real Zcash proposal construction, compares privacy behavior across baseline and candidate implementations, and turns regressions into deterministic replay cases.

ShieldSpec is a **Core & Tooling** project. The privacy oracle remains usable as a lightweight Rust library, CLI, and GitHub Action; the fuzzing layer adds a real-wallet proposal harness, deterministic semantic scenario generation, differential runner isolation, shrinking, and a replayable discovery corpus.

## Developer product and first steps

ShieldSpec is being built as lasting open-source developer infrastructure, beyond the Zecathon submission. Start with the [developer quickstart](docs/getting-started.md), review the [product roadmap and current implementation boundaries](docs/roadmap.md), or preview the [responsive static website](site/README.md). The website includes a read-only historical-evidence explorer and does not pretend to run live Zcash fuzzing in a browser. GitHub Pages is configured for `https://ay-obami.github.io/ShieldSpec/`, with publishing gated on a verified workflow after the site changes reach `main`.

## Reproduced Zcash wallet privacy history and ShieldSpec's fuzzer demo

**Real historical privacy-policy difference:** The same test-only Rust witness invokes real `zallet-core` privacy-policy code at revisions [`f496c9b`](https://github.com/zcash/zallet/commit/f496c9b6b2c6ebb0065b329c11bda4a5fdf73568) and [`e966780`](https://github.com/zcash/zallet/commit/e966780216b7a86b0a1a6618f2fde5b574da7171), from [zallet#677](https://github.com/zcash/zallet/pull/677). Under `FullPrivacy`, the affected code **accepted Ironwood→Sapling** shielded-pool crossing (which exposes a public value balance), while the fixed code **rejects** it. Pinned sources, control tests, both raw logs and a reproducible CLI are in [the Zallet privacy-history evidence](evidence/historical-zallet-677/README.md). It is a previously documented and fixed upstream wallet-core issue, not a new discovery or an automatically found ShieldSpec fuzzer result.

**Cross-pool snapshot detection:** ShieldSpec now reports `SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE` when normalized Zcash proposals move public net balances between Sapling, Orchard, or Ironwood. Real proposal tests cover both observed crossings and an SDK change-pool preference that does **not** result in an actual crossing. This snapshot-level warning is valuable to a developer's privacy regression policy, but it cannot by itself prove a real wallet's `FullPrivacy` policy verdict. Historical Zallet acceptance/rejection still comes from the independent pinned witness, not automatic discovery by the general fuzzer.

A separate [librustzcash#2614 reproduction](evidence/historical-2614/README.md) demonstrates a genuine SDK **correctness** difference in transparent send-max proposals; it is not itself evidence of a privacy leak. The ShieldSpec CLI privacy-differential demo below injects a labeled fault to validate discovery, minimization and replay without misrepresenting it as upstream behavior.

To reproduce the privacy-policy comparison (Rust 1.95, network access, upstream build dependencies):

```bash
python3 scripts/reproduce_zallet_677.py --output evidence/historical-zallet-677/results
```

For a judge-facing sequence, see the [submission summary](docs/submission.md), [captioned 1080p demo video](docs/media/shieldspec-zecathon-captioned-demo.mp4), and [optional narrated filming plan](docs/demo-video-script.md). The captioned video is generated from verified historical logs and executed CLI output; it has **no voiceover**. To regenerate it after building `shieldspec-fuzz`, run `python3 scripts/render_demo_video.py` (requires Pillow and ffmpeg).

## Quick judge demo

The shortest end-to-end demonstration is:

```bash
cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- demo --format human
```

The command intentionally exits with status `2` after showing:

- a baseline produced from real Zcash wallet state and the real proposal engine;
- zero baseline privacy findings;
- a candidate with a **deliberately injected demo-only transparent-change fault**;
- `SS003_TRANSPARENT_CHANGE` and `SS005_PRIVACY_BOUNDARY`;
- a semantically minimized failing scenario; and
- an exact `replay` command.

The demo fault is controlled evidence that the differential pipeline works. **It is not presented as a discovered librustzcash vulnerability.**

The current deterministic demo reproduces with:

```bash
cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- \
  replay \
  --seed 148 \
  --ordinal 0 \
  --policy standard \
  --demo-regression transparent-change \
  --format human
```

That replay also exits `2` and reproduces the same differential findings.

## What is real in the fuzzer

ShieldSpec does not fuzz serialized proposal JSON and call that wallet testing.

For each generated scenario, the Zcash runner:

1. creates an isolated in-memory Zcash wallet;
2. creates synthetic compact blocks containing wallet-owned shielded funds;
3. scans those blocks through librustzcash wallet code;
4. applies the generated confirmation depth and wallet-state parameters;
5. invokes the real `propose_standard_transfer` proposal path;
6. converts the resulting real `Proposal` through `shieldspec-zcash`; and
7. sends only the normalized `ProposalSnapshot` across the differential boundary.

The chain fixture is synthetic and deterministic. Input selection, fees, change, selected pools, multi-step construction, and proposal structure come from the pinned Zcash wallet machinery.

The runner includes a real ZIP 320 shielded-to-TEX case. librustzcash constructs the expected two-step flow: an ephemeral transparent bridge output in step 0 is referenced as a prior-step input in step 1. ShieldSpec preserves that provenance so the internal bridge is not confused with ordinary wallet transparent funding or change.

## Differential architecture

Baseline and candidate runners are separate processes:

```text
                  same Scenario JSON
                       /      \
                      /        \
              baseline runner  candidate runner
                    |               |
              real Proposal    real Proposal
                    |               |
             ProposalSnapshot ProposalSnapshot
                      \       /
                       \     /
                    ShieldSpec
                  privacy oracle
                       |
              regression / pass
                       |
                 shrink + replay
```

No librustzcash Rust type crosses that process boundary. A baseline runner and candidate runner can therefore be compiled independently against different SDK versions or wallet branches while sharing the same stable scenario and snapshot contracts.

A difference in proposal structure is not automatically a privacy failure. ShieldSpec fails the differential only when the candidate introduces a new policy finding or worsens an existing structural finding.

Operational runner errors are reported separately from privacy regressions.

## Deterministic fuzzing

Build the runner binaries:

```bash
cargo build --locked -p shieldspec-zcash-runner --bins
```

A self-comparison smoke campaign should produce no regression:

```bash
target/debug/shieldspec-fuzz fuzz \
  --baseline-runner target/debug/shieldspec-zcash-runner \
  --candidate-runner target/debug/shieldspec-zcash-runner \
  --seed 148 \
  --cases 2 \
  --format human
```

For a real differential campaign, point `--baseline-runner` and `--candidate-runner` at independently built compatible runner executables. Both receive the exact same scenario JSON.

Generated dimensions currently include shielded funding pool, fallback change pool, note count, note values, payment value, confirmation depth, extra blocks, recipient identity, ordinary shielded payments, and ZIP 320 TEX payments.

A reported case contains its seed and ordinal. The semantic shrinker removes unnecessary state while repeatedly executing the real proposal path and preserving the target privacy regression.

With `fuzz --save <discovery.json>`, ShieldSpec also saves each executed runner binary's SHA-256, its exact arguments and canonical path, the policy, and optional **declared** SDK revision labels (`--baseline-sdk` and `--candidate-sdk`). `replay --input <discovery.json>` restores those runner identities, checks the hashes and replays the minimized scenario. A changed binary or conflicting policy fails closed. Runner hashes prove **which bytes ran**; SDK revision labels must still be verified against build records. The historical reproducer verifies actual upstream Git revisions.

## Replay corpus

Replayable minimized cases live under:

```text
crates/shieldspec-zcash-runner/fixtures/discoveries/
```

For example:

```bash
cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- \
  replay \
  --input crates/shieldspec-zcash-runner/fixtures/discoveries/tex-transparent-recipient.json \
  --format human
```

The committed TEX case is a **known expected Zcash behavior**, not a claimed upstream vulnerability: it preserves the transparent recipient finding while proving the ZIP 320 ephemeral bridge is not mislabeled as wallet transparent funding or transparent change. A future differential finding saved with `--save` uses the same seed/ordinal/original/minimized replay format, extended with optional runner provenance. Pre-provenance corpus files remain readable.

## Published motivation

ShieldSpec is grounded in public Zcash developer-testing gaps, without claiming to close those upstream issues:

- [zcash/zallet#693](https://github.com/zcash/zallet/issues/693) documents that realistic `Step` and `Proposal` values can be cumbersome enough to block direct privacy-policy unit testing.
- [zcash/librustzcash#2614](https://github.com/zcash/librustzcash/issues/2614) documents proposal-construction edge cases that escaped existing tests because the exercised path did not reach the affected behavior.
- [zcash/librustzcash#2775](https://github.com/zcash/librustzcash/issues/2775) explicitly calls for privacy `leak-regression` tests where ordinary correctness assertions did not verify the privacy property itself.

Those issues validate the problem; they are not the product. ShieldSpec's contribution is the reusable workflow around them: generate realistic wallet conditions, let the actual Zcash proposal machinery construct proposals, evaluate explicit privacy invariants, compare implementations, and minimize failures into replayable regression cases.

## Original ShieldSpec regression gate

The original deterministic snapshot workflow remains available:

```bash
cargo run --locked -p shieldspec-cli -- \
  diff \
  --baseline fixtures/shielded-payment.json \
  --candidate fixtures/transparent-change.json \
  --policy standard \
  --format human
```

It reports `SS003_TRANSPARENT_CHANGE` and exits `2`.

The GitHub Action can run the same policy gate:

```yaml
- name: Check proposal privacy regression
  uses: Ay-obami/ShieldSpec@v0.1.0
  with:
    baseline: fixtures/shielded-payment.json
    candidate: fixtures/transparent-change.json
    policy: standard
    format: sarif
```

The Action's `status` output is `0` for pass, `1` for operational/input error, and `2` for privacy failure.

## Architecture

- `shieldspec-core`: normalized proposal model, privacy rules, assessment, validation, and structural regression comparison.
- `shieldspec-testkit`: builders and canonical privacy fixtures.
- `shieldspec-zcash`: adapter from supported real Zcash SDK `Proposal` structures to stable snapshots.
- `shieldspec-fuzz`: SDK-independent process orchestration and differential oracle.
- `shieldspec-zcash-runner`: non-published real-wallet test harness, deterministic scenario generator, shrinker, replay corpus, and `shieldspec-fuzz` binary.
- `shieldspec-cli`: lightweight snapshot `check`, `diff`, and fixture commands.
- `action.yml`: reusable GitHub Action for the lightweight regression gate.
- `examples/mock-wallet`: independent downstream consumer example.

## Privacy model and limits

ShieldSpec evaluates proposal facts and configured invariants. It can report transparent funding, recipients or change; unknown routing; transparent/shielded boundary transitions; and regressions between two proposal outcomes.

It does not sign or broadcast transactions, deanonymize users, inspect peer-to-peer or IP metadata, calculate an anonymity score, or prove that a transaction is private.

A clean fuzz campaign means only:

> no configured differential privacy regression was found in the scenarios that were exercised.

It does **not** guarantee privacy or anonymity, and fuzzing does not guarantee that every possible regression will be discovered.

The real-proposal harness uses isolated testing wallet state; it is not a mainnet wallet and does not require user seed phrases or custody.

See [docs/privacy-model.md](docs/privacy-model.md) for the evidence boundary and [docs/demo.md](docs/demo.md) for the judge-facing flow.

## Development

The repository pins Rust 1.95.0.

Run the complete workspace verification:

```bash
cargo fmt --check
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Run only the fuzzing/real-proposal layer:

```bash
cargo test --locked -p shieldspec-fuzz -p shieldspec-zcash-runner --all-targets
cargo clippy --locked -p shieldspec-fuzz -p shieldspec-zcash-runner --all-targets -- -D warnings
```

The CI suite also runs a deterministic clean differential campaign, the judge demo, exact demo differential replay, and committed replay-corpus verification.
