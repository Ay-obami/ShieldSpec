use serde_json::json;
use shieldspec_fuzz::{
    parse_runner_response, RunnerProtocolViolation, RunnerResponse, WalletPolicyDecision,
    WalletPolicyVerdict, RUNNER_PROTOCOL_VERSION,
};
use shieldspec_testkit::fixture;

#[test]
fn version_one_observation_preserves_the_wallet_verdict() {
    let expected = RunnerResponse::Observed {
        snapshot: fixture("shielded-payment").expect("snapshot"),
        wallet_policy: WalletPolicyVerdict {
            policy: "zcash_spend_orchard_only_v1".to_owned(),
            decision: WalletPolicyDecision::Rejected,
        },
    };
    let mut wire = serde_json::to_value(&expected).expect("response JSON");
    wire["protocol_version"] = json!(1);

    assert_eq!(RUNNER_PROTOCOL_VERSION, 1);
    assert_eq!(
        parse_runner_response(&serde_json::to_vec(&wire).expect("wire JSON")).expect("v1 wire"),
        expected
    );
}

#[test]
fn unversioned_legacy_response_remains_compatible() {
    let expected = RunnerResponse::Ok {
        snapshot: fixture("shielded-payment").expect("snapshot"),
    };
    let wire = serde_json::to_vec(&expected).expect("legacy JSON");
    assert_eq!(parse_runner_response(&wire).expect("legacy wire"), expected);
}

#[test]
fn unsupported_version_is_a_typed_protocol_violation() {
    let response = RunnerResponse::Ok {
        snapshot: fixture("shielded-payment").expect("snapshot"),
    };
    let mut wire = serde_json::to_value(response).expect("response JSON");
    wire["protocol_version"] = json!(2);

    let error = parse_runner_response(&serde_json::to_vec(&wire).expect("wire JSON"))
        .expect_err("the next incompatible protocol cannot silently pass");
    assert_eq!(error.code(), "unsupported_runner_protocol_version");
    assert_eq!(
        error,
        RunnerProtocolViolation::UnsupportedVersion {
            found: 2,
            supported: 1
        }
    );
}

#[test]
fn non_integer_version_is_a_typed_protocol_violation() {
    let response = RunnerResponse::Ok {
        snapshot: fixture("shielded-payment").expect("snapshot"),
    };
    let mut wire = serde_json::to_value(response).expect("response JSON");
    wire["protocol_version"] = json!("1");

    let error = parse_runner_response(&serde_json::to_vec(&wire).expect("wire JSON"))
        .expect_err("the version must be a JSON integer");
    assert_eq!(error, RunnerProtocolViolation::InvalidVersion);
    assert_eq!(error.code(), "invalid_runner_protocol_version");
}

#[test]
fn empty_policy_name_is_a_typed_protocol_violation() {
    let response = RunnerResponse::Observed {
        snapshot: fixture("shielded-payment").expect("snapshot"),
        wallet_policy: WalletPolicyVerdict {
            policy: "\t ".to_owned(),
            decision: WalletPolicyDecision::Accepted,
        },
    };
    let wire = serde_json::to_vec(&response).expect("response JSON");

    let error = parse_runner_response(&wire).expect_err("a blank policy is not observable");
    assert_eq!(error, RunnerProtocolViolation::EmptyWalletPolicy);
    assert_eq!(error.code(), "empty_wallet_policy");
}

#[test]
fn malformed_observation_has_a_distinct_json_protocol_code() {
    let wire = json!({
        "status": "observed",
        "snapshot": fixture("shielded-payment").expect("snapshot"),
        "wallet_policy": {
            "policy": "zcash_spend_orchard_only_v1",
            "decision": "maybe"
        },
        "protocol_version": 1
    });

    let error = parse_runner_response(&serde_json::to_vec(&wire).expect("wire JSON"))
        .expect_err("unknown decision cannot count as a wallet verdict");
    assert_eq!(error.code(), "invalid_runner_json");
}
