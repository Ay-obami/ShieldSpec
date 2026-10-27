#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process,
};

use serde_json::{json, Value};
use shieldspec_core::Policy;
use shieldspec_fuzz::{
    run_differential, DifferentialError, RunnerCommand, RunnerResponse, RunnerSide,
};
use shieldspec_testkit::fixture;

fn temp_path(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("shieldspec-fuzz-{}-{name}", process::id()));
    fs::create_dir_all(&root).expect("create test dir");
    root
}

fn fake_runner(
    root: &Path,
    name: &str,
    response: &RunnerResponse,
    exit_code: i32,
) -> (RunnerCommand, PathBuf) {
    let response_path = root.join(format!("{name}.json"));
    fs::write(
        &response_path,
        serde_json::to_vec(response).expect("serialize runner response"),
    )
    .expect("write response");

    let input_path = root.join(format!("{name}-stdin.json"));
    let script_path = root.join(format!("{name}.sh"));
    fs::write(
        &script_path,
        format!(
            "#!/bin/sh\ncat > '{}'\ncat '{}'\nexit {exit_code}\n",
            input_path.display(),
            response_path.display()
        ),
    )
    .expect("write fake runner");
    let mut permissions = fs::metadata(&script_path)
        .expect("runner metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("make runner executable");

    // Invoke a stable shell executable: directly exec'ing a freshly written
    // script can sporadically return ETXTBSY during parallel test execution.
    (
        RunnerCommand::new("/bin/sh").arg(script_path.to_string_lossy().into_owned()),
        input_path,
    )
}

fn captured_scenario(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read captured scenario"))
        .expect("captured stdin is JSON")
}

#[test]
fn candidate_only_privacy_regression_is_reported_through_process_boundary() {
    let root = temp_path("privacy-regression");
    let (baseline, baseline_input) = fake_runner(
        &root,
        "baseline",
        &RunnerResponse::Ok {
            snapshot: fixture("shielded-payment").expect("baseline"),
        },
        0,
    );
    let (candidate, candidate_input) = fake_runner(
        &root,
        "candidate",
        &RunnerResponse::Ok {
            snapshot: fixture("transparent-change").expect("candidate"),
        },
        0,
    );
    let scenario = json!({
        "kind": "tex_payment",
        "params": {
            "funding_zat": 100000,
            "payment_zat": 50000
        }
    });

    let report = run_differential(&baseline, &candidate, &scenario, &Policy::standard())
        .expect("differential execution");

    assert!(!report.regression.passes);
    assert!(report
        .regression
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS003_TRANSPARENT_CHANGE"));
    assert_eq!(captured_scenario(&baseline_input), scenario);
    assert_eq!(captured_scenario(&candidate_input), scenario);
}

#[test]
fn two_independent_runner_commands_receive_identical_scenario_json() {
    let root = temp_path("same-scenario");
    let snapshot = fixture("shielded-payment").expect("snapshot");
    let (baseline, baseline_input) = fake_runner(
        &root,
        "baseline",
        &RunnerResponse::Ok {
            snapshot: snapshot.clone(),
        },
        0,
    );
    let (candidate, candidate_input) =
        fake_runner(&root, "candidate", &RunnerResponse::Ok { snapshot }, 0);
    let scenario = json!({
        "seed": 42,
        "kind": "orchard_payment",
        "params": {"funding_zat": 120000}
    });

    let report = run_differential(&baseline, &candidate, &scenario, &Policy::standard())
        .expect("differential execution");

    assert!(report.regression.passes);
    assert_eq!(
        report.require_comparable_wallet_policy_observations(),
        Err(shieldspec_fuzz::MissingWalletPolicyObservations::Both)
    );
    assert_eq!(captured_scenario(&baseline_input), scenario);
    assert_eq!(captured_scenario(&candidate_input), scenario);
}

#[test]
fn benign_structural_reordering_does_not_fail_the_differential_oracle() {
    let root = temp_path("benign-structure");
    let baseline_snapshot = fixture("transparent-change").expect("baseline");
    let mut candidate_snapshot = baseline_snapshot.clone();
    candidate_snapshot.steps[0].outputs.reverse();

    let (baseline, _) = fake_runner(
        &root,
        "baseline",
        &RunnerResponse::Ok {
            snapshot: baseline_snapshot,
        },
        0,
    );
    let (candidate, _) = fake_runner(
        &root,
        "candidate",
        &RunnerResponse::Ok {
            snapshot: candidate_snapshot,
        },
        0,
    );

    let report = run_differential(
        &baseline,
        &candidate,
        &json!({"kind":"same-semantics"}),
        &Policy::standard(),
    )
    .expect("differential execution");

    assert!(
        report.regression.passes,
        "reordering existing facts is not a new privacy regression"
    );
    assert!(report.regression.findings.is_empty());
}

#[test]
fn candidate_operational_failure_is_not_misreported_as_privacy_regression() {
    let root = temp_path("operational");
    let (baseline, _) = fake_runner(
        &root,
        "baseline",
        &RunnerResponse::Ok {
            snapshot: fixture("shielded-payment").expect("baseline"),
        },
        0,
    );
    let (candidate, _) = fake_runner(
        &root,
        "candidate",
        &RunnerResponse::Error {
            message: "proposal engine rejected scenario".to_owned(),
        },
        1,
    );

    let error = run_differential(
        &baseline,
        &candidate,
        &json!({"kind":"test"}),
        &Policy::standard(),
    )
    .expect_err("candidate must fail operationally");

    assert_eq!(error.side(), RunnerSide::Candidate);
    assert!(matches!(error, DifferentialError::Operational { .. }));
}

#[test]
fn genuine_wallet_policy_decision_difference_is_reported_without_snapshot_change() {
    use shieldspec_fuzz::{WalletPolicyDecision, WalletPolicyVerdict};

    let root = temp_path("wallet-decision-difference");
    let snapshot = fixture("shielded-payment").expect("snapshot");
    let (baseline, _) = fake_runner(
        &root,
        "baseline",
        &RunnerResponse::Observed {
            snapshot: snapshot.clone(),
            wallet_policy: WalletPolicyVerdict {
                policy: "test_policy".to_owned(),
                decision: WalletPolicyDecision::Rejected,
            },
        },
        0,
    );
    let (candidate, _) = fake_runner(
        &root,
        "candidate",
        &RunnerResponse::Observed {
            snapshot,
            wallet_policy: WalletPolicyVerdict {
                policy: "test_policy".to_owned(),
                decision: WalletPolicyDecision::Accepted,
            },
        },
        0,
    );

    let report = run_differential(
        &baseline,
        &candidate,
        &json!({"scenario": 1}),
        &Policy::standard(),
    )
    .expect("both runners made normal wallet-policy decisions");

    assert!(
        report.regression.passes,
        "snapshots are structurally identical"
    );
    assert!(
        report.has_difference(),
        "a changed wallet decision must be discoverable"
    );
    assert_eq!(
        report.require_comparable_wallet_policy_observations(),
        Ok(())
    );
    let difference = report.wallet_policy_difference.expect("decision evidence");
    assert_eq!(difference.policy, "test_policy");
    assert_eq!(difference.baseline, WalletPolicyDecision::Rejected);
    assert_eq!(difference.candidate, WalletPolicyDecision::Accepted);
}

#[test]
fn uninstrumented_runner_does_not_invent_a_wallet_policy_verdict() {
    use shieldspec_fuzz::{WalletPolicyDecision, WalletPolicyVerdict};

    let root = temp_path("uninstrumented-runner");
    let snapshot = fixture("shielded-payment").expect("snapshot");
    let (baseline, _) = fake_runner(
        &root,
        "baseline",
        &RunnerResponse::Ok {
            snapshot: snapshot.clone(),
        },
        0,
    );
    let (candidate, _) = fake_runner(
        &root,
        "candidate",
        &RunnerResponse::Observed {
            snapshot,
            wallet_policy: WalletPolicyVerdict {
                policy: "test_policy".to_owned(),
                decision: WalletPolicyDecision::Accepted,
            },
        },
        0,
    );

    let report = run_differential(
        &baseline,
        &candidate,
        &json!({"scenario": 1}),
        &Policy::standard(),
    )
    .expect("runner protocol");
    assert!(!report.has_difference());
    assert!(report.wallet_policy_difference.is_none());
    assert_eq!(
        report.require_comparable_wallet_policy_observations(),
        Err(shieldspec_fuzz::MissingWalletPolicyObservations::Baseline)
    );
    assert!(report.baseline_wallet_policy.is_none());
    assert_eq!(
        report.candidate_wallet_policy,
        Some(WalletPolicyVerdict {
            policy: "test_policy".to_owned(),
            decision: WalletPolicyDecision::Accepted,
        })
    );
}

#[test]
fn differently_named_wallet_policies_cannot_be_compared_as_if_equivalent() {
    use shieldspec_fuzz::{WalletPolicyDecision, WalletPolicyVerdict};

    let root = temp_path("incomparable-policies");
    let snapshot = fixture("shielded-payment").expect("fixture");
    let (baseline, _) = fake_runner(
        &root,
        "baseline",
        &RunnerResponse::Observed {
            snapshot: snapshot.clone(),
            wallet_policy: WalletPolicyVerdict {
                policy: "wallet_standard".to_owned(),
                decision: WalletPolicyDecision::Accepted,
            },
        },
        0,
    );
    let (candidate, _) = fake_runner(
        &root,
        "candidate",
        &RunnerResponse::Observed {
            snapshot,
            wallet_policy: WalletPolicyVerdict {
                policy: "wallet_strict".to_owned(),
                decision: WalletPolicyDecision::Rejected,
            },
        },
        0,
    );

    let error = run_differential(
        &baseline,
        &candidate,
        &json!({"scenario": 1}),
        &Policy::standard(),
    )
    .expect_err("different wallet policies cannot be called a clean comparison");
    assert!(matches!(
        error,
        DifferentialError::Protocol {
            side: RunnerSide::Candidate,
            ..
        }
    ));
}

#[test]
fn unsupported_runner_version_is_caught_across_the_process_boundary() {
    let root = temp_path("unsupported-runner-version");
    let snapshot = fixture("shielded-payment").expect("snapshot");
    let (baseline, _) = fake_runner(
        &root,
        "baseline",
        &RunnerResponse::Ok {
            snapshot: snapshot.clone(),
        },
        0,
    );
    let (candidate, _) = fake_runner(&root, "candidate", &RunnerResponse::Ok { snapshot }, 0);
    let response_path = root.join("candidate.json");
    let mut wire: Value = serde_json::from_slice(&fs::read(&response_path).expect("response"))
        .expect("JSON response");
    wire["protocol_version"] = json!(2);
    fs::write(
        &response_path,
        serde_json::to_vec(&wire).expect("versioned JSON"),
    )
    .expect("rewrite candidate wire response");

    let error = run_differential(
        &baseline,
        &candidate,
        &json!({"scenario": 1}),
        &Policy::standard(),
    )
    .expect_err("unsupported candidate version must fail closed");

    assert!(matches!(
        error,
        DifferentialError::Protocol {
            side: RunnerSide::Candidate,
            ..
        }
    ));
    assert!(
        error
            .to_string()
            .contains("unsupported_runner_protocol_version"),
        "{error}"
    );
}

#[test]
fn empty_observed_wallet_policy_is_a_protocol_violation_not_an_operational_failure() {
    use shieldspec_fuzz::{WalletPolicyDecision, WalletPolicyVerdict};

    let root = temp_path("empty-wallet-policy");
    let snapshot = fixture("shielded-payment").expect("fixture");
    let (baseline, _) = fake_runner(
        &root,
        "baseline",
        &RunnerResponse::Ok {
            snapshot: snapshot.clone(),
        },
        0,
    );
    let (candidate, _) = fake_runner(
        &root,
        "candidate",
        &RunnerResponse::Observed {
            snapshot,
            wallet_policy: WalletPolicyVerdict {
                policy: " \t ".to_owned(),
                decision: WalletPolicyDecision::Rejected,
            },
        },
        0,
    );

    let error = run_differential(
        &baseline,
        &candidate,
        &json!({"scenario": 1}),
        &Policy::standard(),
    )
    .expect_err("a blank named policy is not an observed decision");
    assert!(
        matches!(
            error,
            DifferentialError::Protocol {
                side: RunnerSide::Candidate,
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn shrinking_must_preserve_an_observed_wallet_verdict_even_if_snapshot_finding_survives() {
    use shieldspec_core::{assess, compare};
    use shieldspec_fuzz::{DifferentialReport, WalletPolicyDecision, WalletPolicyDifference};

    let baseline_snapshot = fixture("shielded-payment").expect("baseline");
    let candidate_snapshot = fixture("transparent-change").expect("candidate");
    let policy = Policy::standard();
    let original = DifferentialReport {
        baseline: assess(&baseline_snapshot, &policy),
        candidate: assess(&candidate_snapshot, &policy),
        regression: compare(&baseline_snapshot, &candidate_snapshot, &policy),
        wallet_policy_difference: Some(WalletPolicyDifference {
            policy: "test_policy".to_owned(),
            baseline: WalletPolicyDecision::Accepted,
            candidate: WalletPolicyDecision::Rejected,
        }),
        baseline_wallet_policy: None,
        candidate_wallet_policy: None,
    };
    assert!(
        !original.regression.passes,
        "also has a snapshot regression"
    );

    let mut changed = original.clone();
    changed.wallet_policy_difference = None;
    assert!(
        !changed.preserves_discovery(&original),
        "cannot discard wallet decision difference during minimization"
    );
    assert!(original.preserves_discovery(&original));
}
