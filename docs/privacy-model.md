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

ShieldSpec infers a privacy regression only under the selected policy: a candidate adds a policy-failing structural finding absent from the baseline, or worsens severity for the same structural finding. Deterministic fingerprints are based on rule, step, and structural evidence rather than free-form messages or value-only changes.

Terms such as “exposure” or “privacy boundary” describe policy interpretations of known proposal structure. They do not establish who controls an address, who a person is, or how observable data will be combined outside the proposal.

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
