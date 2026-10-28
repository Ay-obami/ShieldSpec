use shieldspec_fuzz::{parse_runner_response, RunnerResponse};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn invoke(args: &[&str], input: &str) -> (bool, Vec<u8>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_runner-adapter"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (output.status.success(), output.stdout)
}

#[test]
fn refuses_unconfigured_wallet_even_with_valid_scenario() {
    let (success, output) = invoke(&[], r#"{"kind":"orchard_payment","params":{}}"#);
    assert!(!success);
    assert!(matches!(
        parse_runner_response(&output).unwrap(),
        RunnerResponse::Error { .. }
    ));
}

#[test]
fn demo_emits_valid_versioned_snapshot_envelope() {
    let (success, output) = invoke(&["--demo"], r#"{"kind":"orchard_payment","params":{}}"#);
    assert!(success);
    let wire: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(wire["protocol_version"], 1);
    match parse_runner_response(&output).unwrap() {
        RunnerResponse::Ok { snapshot } => {
            assert!(
                shieldspec_core::assess(&snapshot, &shieldspec_core::Policy::standard()).passes
            );
        }
        _ => panic!("expected a synthetic snapshot"),
    }
}

#[test]
fn rejects_invalid_json_operationally_even_in_demo() {
    let (success, output) = invoke(&["--demo"], "not json");
    assert!(!success);
    assert!(matches!(
        parse_runner_response(&output).unwrap(),
        RunnerResponse::Error { .. }
    ));
}

#[test]
fn rejects_unknown_scenario_kind() {
    let (success, output) = invoke(&["--demo"], r#"{"kind":"unknown","params":{}}"#);
    assert!(!success);
    assert!(matches!(
        parse_runner_response(&output).unwrap(),
        RunnerResponse::Error { .. }
    ));
}
