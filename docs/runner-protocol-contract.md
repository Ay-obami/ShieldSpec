# Runner JSON compatibility contract

The `shieldspec-fuzz` process boundary accepts two response formats:

- **Legacy**: a JSON object with `status: "ok"`, `"observed"`, or `"error"`, with no `protocol_version`. This format remains supported.
- **Explicit v1**: the same object with `"protocol_version": 1` at the top level. All existing fields retain their meanings. This field is optional, including for observed verdicts.

An observed decision has a complete `snapshot` and a `wallet_policy` object containing a nonblank named `policy` and a `decision` of `"accepted"` or `"rejected"`. It represents a wallet policy invocation, not an inference from the snapshot. A legitimate rejection is a successful observation (exit 0); a process/SDK failure must instead return `status: "error"` and fail operationally.

For example, the outer object of a v1 observation is:

```json
{
  "protocol_version": 1,
  "status": "observed",
  "snapshot": {"schema_version": 1, "network": "mainnet", "steps": []},
  "wallet_policy": {"policy": "example_policy", "decision": "rejected"}
}
```

The displayed snapshot is an envelope example, not a proposed transaction. Real runners must provide their complete normalized snapshot.

## Failure and comparison boundaries

`parse_runner_response` returns typed `RunnerProtocolViolation` values with stable `code()` identifiers:

| Code | Meaning |
| --- | --- |
| `invalid_runner_json` | Malformed JSON or response schema/decision |
| `invalid_runner_protocol_version` | A provided version is not a JSON unsigned integer |
| `unsupported_runner_protocol_version` | A provided integer version differs from v1 |
| `empty_wallet_policy` | An observed verdict has a blank policy name |

Process-level invalid response data maps to `DifferentialError::Protocol` with the code prefixed in its message. Valid `status: "error"` and successful-looking output with a nonzero exit are `DifferentialError::Operational`, not wallet rejections. Different policy names between observed baseline and candidate are a candidate-side protocol mismatch.

An uninstrumented `status: "ok"` response supports snapshot comparison only. It never supplies an invented wallet verdict. `DifferentialReport::require_comparable_wallet_policy_observations()` is an optional strict check when a caller requires both observed policy decisions; it returns `MissingWalletPolicyObservations` (baseline, candidate, or both missing) with the stable code `missing_wallet_policy_observations`. Existing callers can continue comparing legacy snapshot-only runs without changing behavior.

## Implemented real SDK adapter and claim boundary

The opt-in Zcash runner `--wallet-spend-policy orchard-only` invokes the pinned SDK's `SpendPolicy::shielded_pools([Orchard])` and emits the named observed policy `zcash_spend_orchard_only_v1`. A rejected restricted proposal can include a diagnostic snapshot produced by an unrestricted proposal attempt; that diagnostic snapshot **does not** override the actual rejected policy verdict. Compare observed decisions only when baseline and candidate name the **same** policy and both executed meaningful proposal paths.

This named SDK spend-source restriction is **not** Zallet's `FullPrivacy` policy. The separate [automatic Zallet #677 integration](zallet-auto-discovery.md) independently compiles two pinned historical versions of Zallet's real `FullPrivacy` policy and feeds verified named observations through this same process protocol. ShieldSpec now detects, minimizes and replays the **already-documented and fixed** Ironwood→Sapling historical difference. The [expanded SDK cases](../crates/shieldspec-zcash-runner/tests/) cover Sapling recipients, Unified Address receiver preferences, shielded change, transparent-UTXO funding, ZIP-317 fee behavior and mixed-pool note selection; those standalone SDK tests are not new privacy-bypass discoveries.

A `status: "observed"` rejection reflects the selected named policy. Rule IDs from analyzing its accompanying `snapshot` describe snapshot properties and must not be reinterpreted as the wallet having accepted a prohibited transaction.
