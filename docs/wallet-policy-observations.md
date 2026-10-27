# Wallet-policy observations in differential fuzzing

ShieldSpec compares two independent kinds of evidence:

1. **Proposal privacy facts** derived by ShieldSpec from a normalized `ProposalSnapshot`.
2. **Wallet-policy decisions** explicitly reported by an instrumented wallet runner.

A ShieldSpec snapshot assessment is **not** a wallet's FullPrivacy, or other wallet-specific,
accept/reject verdict. The standard `shieldspec-zcash-runner` reports proposal snapshots by
default. Its opt-in `--wallet-spend-policy orchard-only` mode additionally executes the
pinned Zcash SDK's **actual Orchard-only spend-source policy** and emits a verdict. This
policy is distinct from Zallet's FullPrivacy policy and is not a test of any historical
FullPrivacy bypass.

## Runner JSON protocol

Existing runners continue returning `{"status":"ok","snapshot":{...}}`.

A wallet integration that really evaluates a named wallet policy may return the following
shape instead. The `snapshot` must be the normal complete `ProposalSnapshot` object:

```json
{
  "status": "observed",
  "snapshot": {
    "schema_version": 1,
    "network": "mainnet",
    "steps": []
  },
  "wallet_policy": {
    "policy": "example_policy",
    "decision": "rejected"
  }
}
```

The sample snapshot demonstrates **the JSON envelope only**, not a valid generated proposal.
Use the real scenario adapter for an actual complete snapshot. `decision` accepts
`accepted` or `rejected`. A rejection is a normal policy observation: return JSON on stdout
and exit zero. Process crashes, malformed input, failure to construct a proposal, or failure
to invoke the wallet policy must use `{"status":"error","message":"..."}` and a failing
exit status; do **not** turn operational failures into `rejected` decisions.

The policy name must be nonempty, and both runners must report the same named policy for
their decisions to be comparable. A mismatch is a protocol error. If either runner emits
only `status: ok`, ShieldSpec performs snapshot checks but makes **no claim** about a
wallet-policy verdict. The differential JSON includes `baseline_wallet_policy` and
`candidate_wallet_policy` when supplied, even when both verdicts agree, so downstream
consumers can distinguish an observed agreement from missing instrumentation.

## Real SDK spend-policy adapter

The opt-in runner flag `--wallet-spend-policy orchard-only` selects a named
`zcash_spend_orchard_only_v1` observation. It calls the pinned
`zcash_client_backend` wallet's real `SpendPolicy::shielded_pools([Orchard])`
proposal path. An adequately funded Orchard wallet is accepted; an otherwise
fundable Sapling-only wallet cannot satisfy the Orchard-only spend restriction
and reports `rejected` from the SDK's typed insufficient-permitted-funds result.

The `snapshot` in an `observed` response remains a complete, genuinely
SDK-generated **unrestricted diagnostic proposal**. In a rejection case, there
is no constrained approved proposal to snapshot. The reported acceptance or
rejection is derived separately from the restricted SDK policy call, never
inferred from that diagnostic snapshot. Invalid scenarios, underfunded
unrestricted proposals and unexpected SDK errors remain operational errors
with nonzero process exit codes.

This mode demonstrates genuine SDK behavior and integration with the general
fuzzer. Comparing the same runner version with itself is a useful negative
control, but neither establishes an upstream bug nor exercises Zallet's
historical FullPrivacy behavior.

## Generic comparison

For a comparable pair, ShieldSpec emits:

```json
"wallet_policy_difference": {
  "policy": "example_policy",
  "baseline": "accepted",
  "candidate": "rejected"
}
```

A changed wallet decision is detectable even when the snapshots are structurally identical.
It is classified as `WALLET_POLICY_DECISION_DIFFERENCE`, independently of the
`SS00x` privacy rules. **Decision difference is not automatically a privacy regression**:
a changed rejection might be a safety improvement, a policy configuration change, or a
correctness change. Human reviewers must interpret the policy contract.

Every campaign result, including `no_regression_found`, reports
`baseline_wallet_policy_observations`,
`candidate_wallet_policy_observations`, and
`comparable_wallet_policy_cases`. A zero comparable count means the campaign
did **not** compare two instrumented wallet policies, even if snapshot checks passed.
The counts refer to generated cases, not extra runner invocations during shrinking.

The `shieldspec-fuzz fuzz` command detects the difference through its normal two-runner
loop, shrinks the generated case while preserving the observed difference, saves a seed,
ordinal, original and minimized scenario, policy, and executable provenance, and exits 2.
`shieldspec-fuzz replay --input <saved.json>` checks the recorded runner identities and
reports whether the **same named policy and decision direction** persist. If the runner
instead produces a different decision change, replay reports
`recorded_rule_reproduced: false` and `status: other_wallet_policy_difference`.
If the original finding disappears entirely, replay reports
`status: recorded_finding_not_reproduced`. A discovery is not saved when
its final minimized execution fails to preserve the original observation.
The discovery rule is not a hardcoded historical bug detector.

## Evidence boundaries

- The integration test uses **synthetic, ordinary** accept/reject outcomes to prove the
  generic detection/minimization/replay machinery; it is not evidence of an upstream Zcash bug.
- Wallet policy enforcement must be executed by the independently built wallet runner.
  Merely labeling a ShieldSpec assessment as a wallet decision is invalid.
- No historical bypass-reproduction experiment is included here.
- A clean run means only that no configured snapshot regression or comparable observed
  decision difference was detected for the scenarios exercised.
