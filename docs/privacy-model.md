# Privacy model

ShieldSpec is a pre-signing regression tester for facts represented in a Zcash transaction proposal. Its conclusions are deliberately limited to that evidence.

## Confirmed facts

When present in the normalized snapshot, ShieldSpec can directly evaluate:

- the declared pool of each input and output;
- whether an output is a recipient, change, or another role;
- step boundaries and step indexes in a multi-step proposal;
- whether transparent value is a real recipient/change output or an internal ephemeral bridge;
- whether an input came from wallet funds or a typed prior-step payment/change output;
- input values, output values, and the proposed fee;
- unknown pool or routing data explicitly preserved by the adapter;
- structural differences between baseline and candidate findings.

Rules report these proposal-level facts. For example, `SS003_TRANSPARENT_CHANGE` means an output explicitly marked as change uses the transparent pool in the supplied candidate.

## Inference

ShieldSpec infers a **snapshot privacy regression** only under the selected policy: a candidate adds a policy-failing structural finding absent from the baseline, or worsens severity for the same structural finding. Deterministic fingerprints are based on rule, step, and structural evidence rather than free-form messages or value-only changes.

An instrumented runner can separately report an **actual wallet-policy accept/reject decision** under a nonempty, named policy. The generic differential engine compares decisions only when both runners report the **same policy name**. A change is labeled `WALLET_POLICY_DECISION_DIFFERENCE` and exits `2`, but it is **not automatically a privacy regression**: wallet policy might have intentionally become stricter, changed configuration, or fixed a correctness issue. If a runner reports only a snapshot, no wallet-policy verdict can be inferred. A policy rejection is a normal observation; failure to construct a proposal or call the wallet policy is an operational error (exit `1`), not a rejection.

Terms such as “exposure” or “privacy boundary” describe policy interpretations of known proposal structure. They do not establish who controls an address, who a person is, or how observable data will be combined outside the proposal.

**Current cross-pool boundary:** `SS005_PRIVACY_BOUNDARY` distinguishes transparent and shielded routes. The new `SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE` detects net value leaving one of Sapling, Orchard, or Ironwood while entering another shielded pool in the same normalized proposal step. It reports the source/destination pools and aggregate candidate crossing amount. These pool-level value balances can be publicly observable even though the individual shielded notes remain private. It is an **observation and policy-rated warning**, not proof that the wallet violated its privacy policy or that the same parties control both ends. Some policies deliberately permit cross-pool transfers. For multiple sources and destinations, the reported crossing amount is a conservative aggregate indication, not a proven pairwise flow attribution.

Likewise, `SS002_TRANSPARENT_FUNDING` may be emitted for an intentional transparent-UTXO shielding transfer. The [real SDK transparent-funding test](../crates/shieldspec-zcash-runner/tests/transparent_funding.rs) exercises this case. Such a configured policy finding does not, by itself, prove that the wallet leaked sensitive information.

Whether a Zallet wallet enforcing `FullPrivacy` wrongly **accepted** such a crossing requires a verdict from **Zallet's actual FullPrivacy policy implementation**. ShieldSpec's generic named-policy verdict protocol and opt-in real SDK `zcash_spend_orchard_only_v1` adapter are on `main` following [PR #42](https://github.com/Ay-obami/ShieldSpec/pull/42). The Orchard-only mode restricts SDK *spend-source selection* and is **not Zallet FullPrivacy**. A restricted SDK rejection may be accompanied by a valid **unrestricted diagnostic proposal snapshot**; that snapshot is not evidence that the restricted proposal was accepted. [PR #43](https://github.com/Ay-obami/ShieldSpec/pull/43) expands real SDK scenarios across Sapling recipients, Unified Address receiver preferences, transparent-UTXO shielding, ZIP-317 fees, and mixed-pool input selection. Those tests exercise genuine SDK proposal behavior under synthetic wallet state; they do not provide a second independent `FullPrivacy` wallet-policy adapter or automatic discovery of Zallet's historical difference.

Our pinned [original Zallet historical witness](../evidence/historical-zallet-677/README.md) independently calls actual policy code in the affected and corrected revisions. A newer, distinct [automatic Zallet integration](zallet-auto-discovery.md) now has the **general fuzzer** construct validated scenarios, query both independently compiled real policy versions, discover the documented Ironwood→Sapling decision difference, minimize it and replay it with SHA-linked provenance. This tests an already-fixed issue using a synthetic no-recipient proposal, not an undisclosed new leak or full wallet payment flow.

The real SDK [mixed-pool tests](../crates/shieldspec-zcash-runner/tests/mixed_pool.rs) also reproduce the publicly documented [librustzcash #3133](https://github.com/zcash/librustzcash/issues/3133) input-selection failure on the pinned SDK, alongside a successful mixed-pool control. That unexpected `InsufficientFunds` result is an **availability/correctness** defect, **not** evidence of a privacy-policy bypass or a newly discovered upstream issue. See [the roadmap](roadmap.md) and [wallet-policy protocol](wallet-policy-observations.md).

## Unknowns

The snapshot does not establish:

- the identity or intent of senders, recipients, or observers;
- wallet history or facts omitted or misclassified by the producer/adapter;
- the final signed transaction if it differs from the proposal;
- mempool, peer-to-peer, IP, timing, device, exchange, or other network metadata;
- external datasets or correlations an observer may possess;
- real-world anonymity or the size/composition of an anonymity set.

Unknown pools or routes fail closed under the configured rules; other absent context remains unknown. It must not be silently promoted to a privacy claim.

## Explicit boundaries

ShieldSpec never needs or handles seed phrases, spending keys, signing authority, or custody. It does not sign or broadcast transactions, connect proposal facts to network deanonymization, or perform public-chain deanonymization. It produces no numeric anonymity score.

A passing assessment means only: **no configured policy failure was found in the supplied proposal facts**. It is not an anonymity guarantee, a security proof, or a statement that the eventual transaction is safe against every observer.

The differential fuzzer inherits the same boundary. A clean campaign means only that no configured baseline-to-candidate regression was found in the generated scenarios that actually ran. Scenario generation is deliberately semantic and biased toward known privacy-sensitive boundaries, but it cannot exhaust all wallet histories, SDK states, policies, or future protocol behavior.
