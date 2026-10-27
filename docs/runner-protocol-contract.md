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
