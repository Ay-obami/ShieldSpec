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

    (RunnerCommand::new(script_path), input_path)
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

    assert!(!report.passes);
    assert!(report
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

    assert!(report.passes);
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
        report.passes,
        "reordering existing facts is not a new privacy regression"
    );
    assert!(report.findings.is_empty());
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
