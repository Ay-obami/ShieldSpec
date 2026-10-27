#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process,
};

use serde_json::json;
use shieldspec_core::Policy;
use shieldspec_fuzz::{
    compare_snapshots, run_differential, DifferentialError, RunnerCommand, RunnerResponse,
    RunnerSide,
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
) -> RunnerCommand {
    let response_path = root.join(format!("{name}.json"));
    fs::write(
        &response_path,
        serde_json::to_vec(response).expect("serialize runner response"),
    )
    .expect("write response");

    let script_path = root.join(format!("{name}.sh"));
    fs::write(
        &script_path,
        format!(
            "#!/bin/sh\ncat >/dev/null\ncat '{}'\nexit {exit_code}\n",
            response_path.display()
        ),
    )
    .expect("write fake runner");
    let mut permissions = fs::metadata(&script_path)
        .expect("runner metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("make runner executable");

    RunnerCommand::new(script_path)
}

#[test]
fn candidate_only_privacy_regression_is_reported_by_rule_id() {
    let baseline = fixture("shielded-payment").expect("baseline");
    let candidate = fixture("transparent-change").expect("candidate");

    let report = compare_snapshots(&baseline, &candidate, &Policy::standard());

    assert!(!report.passes);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS003_TRANSPARENT_CHANGE"));
}

#[test]
fn two_independent_runner_commands_receive_the_same_scenario() {
    let root = temp_path("same-scenario");
    let baseline_snapshot = fixture("shielded-payment").expect("baseline");
    let candidate_snapshot = baseline_snapshot.clone();

    let baseline = fake_runner(
        &root,
        "baseline",
        &RunnerResponse::Ok {
            snapshot: baseline_snapshot,
        },
        0,
    );
    let candidate = fake_runner(
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
        &json!({"kind":"orchard_payment","params":{"funding_zat":120000}}),
        &Policy::standard(),
    )
    .expect("differential execution");

    assert!(report.passes);
}

#[test]
fn candidate_operational_failure_is_not_misreported_as_privacy_regression() {
    let root = temp_path("operational");
    let baseline = fake_runner(
        &root,
        "baseline",
        &RunnerResponse::Ok {
            snapshot: fixture("shielded-payment").expect("baseline"),
        },
        0,
    );
    let candidate = fake_runner(
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
