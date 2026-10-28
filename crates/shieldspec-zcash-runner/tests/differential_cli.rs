#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{self, Command},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use shieldspec_fuzz::{RunnerResponse, WalletPolicyDecision, WalletPolicyVerdict};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(purpose: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("wall clock")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("shieldspec-{purpose}-{}-{nonce}", process::id()));
        fs::create_dir(&path).expect("create unique test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

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
    let directory = TestDirectory::new("verdict-cli");
    let root = directory.path();
    let baseline = observed_runner(root, "baseline", WalletPolicyDecision::Accepted);
    let candidate = observed_runner(root, "candidate", WalletPolicyDecision::Rejected);
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
    assert_eq!(changed_replay["status"], "other_wallet_policy_difference");

    let now_agree = RunnerResponse::Observed {
        snapshot: serde_json::from_str(include_str!("../../../fixtures/shielded-payment.json"))
            .expect("fixture"),
        wallet_policy: WalletPolicyVerdict {
            policy: "test_policy".to_owned(),
            decision: WalletPolicyDecision::Rejected,
        },
    };
    fs::write(
        root.join("candidate.json"),
        serde_json::to_vec(&now_agree).expect("json"),
    )
    .expect("update observation");
    let (no_difference_exit, no_difference_replay) = run_binary(&[
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
    assert_eq!(no_difference_exit, 0, "{no_difference_replay}");
    assert_eq!(no_difference_replay["recorded_rule_reproduced"], false);
    assert_eq!(
        no_difference_replay["status"],
        "recorded_finding_not_reproduced"
    );
}

#[test]
fn clean_campaign_reports_real_wallet_policy_coverage_even_without_a_finding() {
    let directory = TestDirectory::new("coverage-cli");
    let root = directory.path();
    let baseline = observed_runner(root, "coverage-baseline", WalletPolicyDecision::Accepted);
    let candidate = observed_runner(root, "coverage-candidate", WalletPolicyDecision::Accepted);

    let run = || {
        run_binary(&[
            "fuzz",
            "--baseline-runner",
            baseline.to_str().expect("path"),
            "--candidate-runner",
            candidate.to_str().expect("path"),
            "--seed",
            "23",
            "--cases",
            "3",
            "--format",
            "json",
        ])
    };

    let (exit, both_observed) = run();
    assert_eq!(exit, 0, "{both_observed}");
    assert_eq!(both_observed["status"], "no_regression_found");
    assert_eq!(both_observed["cases_executed"], 3);
    assert_eq!(both_observed["baseline_wallet_policy_observations"], 3);
    assert_eq!(both_observed["candidate_wallet_policy_observations"], 3);
    assert_eq!(both_observed["comparable_wallet_policy_cases"], 3);

    // A legacy snapshot-only runner must not be reported as policy-covered.
    let legacy_response = RunnerResponse::Ok {
        snapshot: serde_json::from_str(include_str!("../../../fixtures/shielded-payment.json"))
            .expect("fixture"),
    };
    fs::write(
        root.join("coverage-candidate.json"),
        serde_json::to_vec(&legacy_response).expect("legacy response"),
    )
    .expect("update response");
    let (legacy_exit, partly_observed) = run();
    assert_eq!(legacy_exit, 0, "{partly_observed}");
    assert_eq!(partly_observed["baseline_wallet_policy_observations"], 3);
    assert_eq!(partly_observed["candidate_wallet_policy_observations"], 0);
    assert_eq!(partly_observed["comparable_wallet_policy_cases"], 0);
}

#[test]
fn transient_policy_difference_is_not_saved_as_a_stable_discovery() {
    let directory = TestDirectory::new("unstable-cli");
    let root = directory.path();
    let baseline = observed_runner(root, "stable-baseline", WalletPolicyDecision::Accepted);
    let _accepted = observed_runner(root, "later-accepted", WalletPolicyDecision::Accepted);
    let _rejected = observed_runner(root, "first-rejected", WalletPolicyDecision::Rejected);
    let seen = root.join("seen");
    let candidate = root.join("unstable-candidate.sh");
    fs::write(
        &candidate,
        format!(
            "#!/bin/sh\ncat >/dev/null\nif [ -f '{}' ]; then cat '{}'; else touch '{}'; cat '{}'; fi\n",
            seen.display(),
            root.join("later-accepted.json").display(),
            seen.display(),
            root.join("first-rejected.json").display(),
        ),
    )
    .expect("write changing runner");
    let mut permissions = fs::metadata(&candidate).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&candidate, permissions).expect("executable");
    let discovery = root.join("must-not-save.json");

    let output = Command::new(env!("CARGO_BIN_EXE_shieldspec-fuzz"))
        .args([
            "fuzz",
            "--baseline-runner",
            baseline.to_str().expect("path"),
            "--candidate-runner",
            candidate.to_str().expect("path"),
            "--seed",
            "77",
            "--cases",
            "1",
            "--save",
            discovery.to_str().expect("path"),
            "--format",
            "json",
        ])
        .output()
        .expect("launch campaign");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no longer reproduces"),
        "unexpected error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !discovery.exists(),
        "a transient finding must never be saved as reproducible evidence"
    );
}

#[test]
fn stale_original_observation_cannot_be_saved_after_successful_shrinking() {
    let directory = TestDirectory::new("stale-original");
    let root = directory.path();
    let baseline = observed_runner(root, "original-baseline", WalletPolicyDecision::Accepted);
    let _accepted = observed_runner(
        root,
        "original-later-accepted",
        WalletPolicyDecision::Accepted,
    );
    let _rejected = observed_runner(
        root,
        "original-first-rejected",
        WalletPolicyDecision::Rejected,
    );
    let original = shieldspec_zcash_runner::ScenarioGenerator::case_at(77, 0).scenario;
    let original_json = serde_json::to_value(original).expect("original JSON");
    let mut original_bytes = serde_json::to_vec(&original_json).expect("canonical original");
    original_bytes.push(b'\n');
    fs::write(root.join("original.json"), original_bytes).expect("original input");
    let seen = root.join("original-seen");
    let candidate = root.join("changing-original.sh");
    fs::write(
        &candidate,
        format!(
            "#!/bin/sh\ncat > '{}'\nif cmp -s '{}' '{}'; then\n if [ -f '{}' ]; then cat '{}'; else touch '{}'; cat '{}'; fi\nelse cat '{}'; fi\n",
            root.join("received.json").display(),
            root.join("received.json").display(),
            root.join("original.json").display(),
            seen.display(),
            root.join("original-later-accepted.json").display(),
            seen.display(),
            root.join("original-first-rejected.json").display(),
            root.join("original-first-rejected.json").display(),
        ),
    )
    .expect("changing runner script");
    let mut permissions = fs::metadata(&candidate).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&candidate, permissions).expect("executable");
    let discovery = root.join("must-not-save.json");

    let output = Command::new(env!("CARGO_BIN_EXE_shieldspec-fuzz"))
        .args([
            "fuzz",
            "--baseline-runner",
            baseline.to_str().expect("path"),
            "--candidate-runner",
            candidate.to_str().expect("path"),
            "--seed",
            "77",
            "--cases",
            "1",
            "--save",
            discovery.to_str().expect("path"),
            "--format",
            "json",
        ])
        .output()
        .expect("launch campaign");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("original scenario no longer reproduces"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!discovery.exists(), "stale original must not be saved");
}

#[test]
fn replay_labels_the_recorded_snapshot_rule_even_if_a_wallet_policy_also_changes() {
    let directory = TestDirectory::new("mixed-replay");
    let root = directory.path();
    let baseline = observed_runner(root, "mixed-baseline", WalletPolicyDecision::Accepted);
    let candidate = observed_runner(root, "mixed-candidate", WalletPolicyDecision::Rejected);

    let mut snapshot: Value =
        serde_json::from_str(include_str!("../../../fixtures/shielded-payment.json"))
            .expect("fixture");
    snapshot["steps"][0]["outputs"][0]["value_zat"] = Value::from(99_900);
    snapshot["steps"][0]["outputs"]
        .as_array_mut()
        .expect("outputs")
        .push(serde_json::json!({
            "pool": "transparent",
            "role": "change",
            "value_zat": 100
        }));
    let candidate_response = RunnerResponse::Observed {
        snapshot: serde_json::from_value(snapshot).expect("candidate snapshot"),
        wallet_policy: WalletPolicyVerdict {
            policy: "test_policy".to_owned(),
            decision: WalletPolicyDecision::Rejected,
        },
    };
    fs::write(
        root.join("mixed-candidate.json"),
        serde_json::to_vec(&candidate_response).expect("runner JSON"),
    )
    .expect("write mixed candidate");
    let generated = shieldspec_zcash_runner::ScenarioGenerator::case_at(77, 0);
    let scenario = generated.scenario;
    let discovery = root.join("recorded-snapshot-discovery.json");
    fs::write(
        &discovery,
        serde_json::to_vec(&serde_json::json!({
            "seed": 77,
            "ordinal": 0,
            "rule_id": "SS003_TRANSPARENT_CHANGE",
            "original": scenario,
            "minimized": scenario
        }))
        .expect("discovery"),
    )
    .expect("write discovery");

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
    assert_eq!(replay["recorded_rule_reproduced"], true, "{replay}");
    assert_eq!(replay["status"], "privacy_regression_reproduced");
    assert_eq!(
        replay["report"]["wallet_policy_difference"]["policy"],
        "test_policy"
    );
}

#[test]
fn saved_ironwood_discovery_replays_minimized_input_with_explicit_runners() {
    let directory = TestDirectory::new("ironwood-legacy-discovery");
    let root = directory.path();
    let baseline = observed_runner(root, "ironwood-base", WalletPolicyDecision::Accepted);
    let candidate = observed_runner(root, "ironwood-candidate", WalletPolicyDecision::Rejected);
    let discovery = root.join("ironwood-discovery.json");
    let scenario = shieldspec_zcash_runner::ScenarioGenerator::nu63_scenario_at(2026, 0);
    let mut minimized = scenario.clone();
    let shieldspec_zcash_runner::Scenario::Nu63OrchardPayment(ref mut payment) = minimized else {
        panic!("expected Ironwood scenario");
    };
    payment.payment.note_count = 1;
    fs::write(
        &discovery,
        serde_json::to_vec(&serde_json::json!({
            "seed": 2026,
            "ordinal": 0,
            "scenario_family": "ironwood",
            "rule_id": "WALLET_POLICY_DECISION_DIFFERENCE",
            "original": scenario,
            "minimized": minimized,
            "wallet_policy_difference": {
                "policy": "test_policy",
                "baseline": "accepted",
                "candidate": "rejected"
            }
        }))
        .expect("discovery JSON"),
    )
    .expect("write legacy-style discovery without executable provenance");

    let (exit, replay) = run_binary(&[
        "replay",
        "--input",
        discovery.to_str().expect("discovery path"),
        "--baseline-runner",
        baseline.to_str().expect("baseline path"),
        "--candidate-runner",
        candidate.to_str().expect("candidate path"),
        "--format",
        "json",
    ]);
    assert_eq!(exit, 2, "{replay}");
    assert_eq!(replay["recorded_rule_reproduced"], true);
    assert_eq!(
        replay["scenario"],
        serde_json::to_value(&minimized).expect("minimized JSON"),
        "saved replay must run the minimized input"
    );
    let reproduction = replay["reproduction"]
        .as_str()
        .expect("reproduction command");
    assert!(
        reproduction.contains("--input") && reproduction.contains("ironwood-discovery.json"),
        "reproducer must use the same saved minimized scenario: {reproduction}"
    );
    assert!(reproduction.contains("--baseline-runner"), "{reproduction}");
    assert!(
        reproduction.contains("--candidate-runner"),
        "{reproduction}"
    );
}

#[test]
fn general_fuzzer_can_generate_and_replay_ironwood_scenarios_by_seed() {
    let directory = TestDirectory::new("ironwood-fuzz");
    let root = directory.path();
    let baseline = observed_runner(root, "ironwood-baseline", WalletPolicyDecision::Accepted);
    let candidate = observed_runner(root, "ironwood-candidate", WalletPolicyDecision::Rejected);
    let discovery = root.join("ironwood-discovery.json");

    let (exit, campaign) = run_binary(&[
        "fuzz",
        "--baseline-runner",
        baseline.to_str().expect("path"),
        "--candidate-runner",
        candidate.to_str().expect("path"),
        "--scenario-family",
        "ironwood",
        "--seed",
        "2026",
        "--cases",
        "1",
        "--save",
        discovery.to_str().expect("path"),
        "--format",
        "json",
    ]);
    assert_eq!(exit, 2, "{campaign}");
    assert_eq!(campaign["scenario_family"], "ironwood");
    assert_eq!(campaign["scenario"]["kind"], "nu63_orchard_payment");
    assert_eq!(
        campaign["minimized_scenario"]["kind"],
        "nu63_orchard_payment"
    );
    let saved: Value =
        serde_json::from_slice(&fs::read(&discovery).expect("discovery")).expect("discovery JSON");
    assert_eq!(saved["scenario_family"], "ironwood");
    assert_eq!(saved["original"]["kind"], "nu63_orchard_payment");

    let (replay_exit, replay) = run_binary(&[
        "replay",
        "--seed",
        "2026",
        "--ordinal",
        "0",
        "--scenario-family",
        "ironwood",
        "--baseline-runner",
        baseline.to_str().expect("path"),
        "--candidate-runner",
        candidate.to_str().expect("path"),
        "--format",
        "json",
    ]);
    assert_eq!(replay_exit, 2, "{replay}");
    assert_eq!(replay["scenario"], campaign["scenario"]);
    assert_eq!(replay["status"], "wallet_policy_difference_reproduced");
}

#[test]
fn zallet_privacy_campaign_requires_comparable_wallet_policy_observations() {
    let directory = TestDirectory::new("zallet-policy-coverage");
    let root = directory.path();
    let baseline = observed_runner(root, "zallet-baseline", WalletPolicyDecision::Accepted);
    let candidate = observed_runner(root, "zallet-candidate", WalletPolicyDecision::Accepted);
    let args = [
        "fuzz",
        "--scenario-family",
        "zallet-privacy",
        "--baseline-runner",
        baseline.to_str().unwrap(),
        "--candidate-runner",
        candidate.to_str().unwrap(),
        "--seed",
        "2026",
        "--cases",
        "3",
        "--format",
        "json",
    ];
    let (exit, report) = run_binary(&args);
    assert_eq!(exit, 0, "{report}");
    assert_eq!(report["cases_executed"], 3);
    assert_eq!(report["comparable_wallet_policy_cases"], 3);

    let snapshot_only = RunnerResponse::Ok {
        snapshot: serde_json::from_str(include_str!("../../../fixtures/shielded-payment.json"))
            .unwrap(),
    };
    fs::write(
        root.join("zallet-candidate.json"),
        serde_json::to_vec(&snapshot_only).unwrap(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_shieldspec-fuzz"))
        .args(args)
        .output()
        .expect("launch Zallet privacy campaign");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("comparable wallet-policy"),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}
