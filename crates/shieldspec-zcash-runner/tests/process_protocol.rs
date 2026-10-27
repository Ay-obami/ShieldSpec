use std::{
    io::Write,
    process::{Command, Stdio},
};

use shieldspec_fuzz::RunnerResponse;
use shieldspec_zcash_runner::{OrchardPaymentScenario, Scenario};

#[test]
fn runner_binary_accepts_scenario_json_and_emits_snapshot_json() {
    let scenario = Scenario::OrchardPayment(OrchardPaymentScenario::demo());
    let mut child = Command::new(env!("CARGO_BIN_EXE_shieldspec-zcash-runner"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start runner");

    serde_json::to_writer(child.stdin.as_mut().expect("stdin"), &scenario).expect("write scenario");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"\n")
        .expect("newline");

    let output = child.wait_with_output().expect("runner output");
    assert!(
        output.status.success(),
        "runner stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let response: RunnerResponse =
        serde_json::from_slice(&output.stdout).expect("runner protocol response");
    let RunnerResponse::Ok { snapshot } = response else {
        panic!("runner returned an operational error");
    };

    assert_eq!(snapshot.network, "local-test");
    assert_eq!(snapshot.steps.len(), 1);
}

#[test]
fn runner_binary_distinguishes_invalid_input_as_operational_error() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shieldspec-zcash-runner"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start runner");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(br#"{"kind":"not_a_real_scenario"}"#)
        .expect("write invalid scenario");

    let output = child.wait_with_output().expect("runner output");
    assert!(!output.status.success());

    let response: RunnerResponse =
        serde_json::from_slice(&output.stdout).expect("runner error response");
    assert!(matches!(response, RunnerResponse::Error { .. }));
}
