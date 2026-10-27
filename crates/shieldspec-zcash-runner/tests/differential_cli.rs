#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{self, Command},
};

use serde_json::Value;
use shieldspec_fuzz::{RunnerResponse, WalletPolicyDecision, WalletPolicyVerdict};

fn observed_runner(root: &Path, name: &str, decision: WalletPolicyDecision) -> PathBuf {
    let payload = root.join(format!("{name}.json"));
    let response = RunnerResponse::Observed {
        snapshot: serde_json::from_str(include_str!("../../../fixtures/shielded-payment.json"))
            .expect("fixture"),
        wallet_policy: WalletPolicyVerdict {
            policy: "test_policy".to_owned(),
            decision,
        },
    };
    fs::write(&payload, serde_json::to_vec(&response).expect("json")).expect("payload");
    let script = root.join(format!("{name}.sh"));
    fs::write(
        &script,
        format!("#!/bin/sh\ncat >/dev/null\ncat '{}'\n", payload.display()),
    )
    .expect("runner script");
    let mut permissions = fs::metadata(&script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script, permissions).expect("executable");
    script
}

fn run_binary(args: &[&str]) -> (u8, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_shieldspec-fuzz"))
        .args(args)
        .output()
        .expect("launch fuzz CLI");
    let body: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON ({error}); stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.code().expect("exit code") as u8, body)
}

#[test]
fn general_fuzzer_minimizes_and_replays_a_wallet_policy_decision_difference() {
    let root = std::env::temp_dir().join(format!("shieldspec-verdict-cli-{}", process::id()));
    fs::create_dir_all(&root).expect("temporary directory");
    let baseline = observed_runner(&root, "baseline", WalletPolicyDecision::Accepted);
    let candidate = observed_runner(&root, "candidate", WalletPolicyDecision::Rejected);
    let discovery = root.join("finding.json");

    let (exit, campaign) = run_binary(&[
        "fuzz",
        "--baseline-runner",
        baseline.to_str().expect("path"),
        "--candidate-runner",
        candidate.to_str().expect("path"),
        "--seed",
        "148",
        "--cases",
        "1",
        "--format",
        "json",
        "--save",
        discovery.to_str().expect("path"),
    ]);

    assert_eq!(exit, 2, "{campaign}");
    assert_eq!(campaign["status"], "wallet_policy_difference");
    assert_eq!(
        campaign["report"]["wallet_policy_difference"]["baseline"],
        "accepted"
    );
    assert_eq!(
        campaign["report"]["wallet_policy_difference"]["candidate"],
        "rejected"
    );
    assert!(campaign["minimized_scenario"].is_object());
    let stored: Value =
        serde_json::from_slice(&fs::read(&discovery).expect("discovery")).expect("discovery JSON");
    assert_eq!(stored["rule_id"], "WALLET_POLICY_DECISION_DIFFERENCE");
    assert_eq!(stored["wallet_policy_difference"]["policy"], "test_policy");
    assert_eq!(stored["wallet_policy_difference"]["baseline"], "accepted");
    assert_eq!(stored["wallet_policy_difference"]["candidate"], "rejected");

    let (exit, replay) = run_binary(&[
        "replay",
        "--input",
        discovery.to_str().expect("path"),
        "--baseline-runner",
        baseline.to_str().expect("path"),
        "--candidate-runner",
        candidate.to_str().expect("path"),
        "--format",
        "json",
    ]);
    assert_eq!(exit, 2, "{replay}");
    assert_eq!(replay["status"], "wallet_policy_difference_reproduced");
    assert_eq!(replay["recorded_rule_reproduced"], true);
    assert_eq!(
        replay["report"]["wallet_policy_difference"]["policy"],
        "test_policy"
    );

    // The binary bytes are unchanged, but the external observation payload changes.
    // A different direction of wallet decision change is NOT the saved finding.
    for (name, decision) in [
        ("baseline", WalletPolicyDecision::Rejected),
        ("candidate", WalletPolicyDecision::Accepted),
    ] {
        let updated = RunnerResponse::Observed {
            snapshot: serde_json::from_str(include_str!("../../../fixtures/shielded-payment.json"))
                .expect("fixture"),
            wallet_policy: WalletPolicyVerdict {
                policy: "test_policy".to_owned(),
                decision,
            },
        };
        fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec(&updated).expect("json"),
        )
        .expect("update observation");
    }
    let (changed_exit, changed_replay) = run_binary(&[
        "replay",
        "--input",
        discovery.to_str().expect("path"),
        "--baseline-runner",
        baseline.to_str().expect("path"),
        "--candidate-runner",
        candidate.to_str().expect("path"),
        "--format",
        "json",
    ]);
    assert_eq!(changed_exit, 2);
    assert_eq!(changed_replay["recorded_rule_reproduced"], false);
}
