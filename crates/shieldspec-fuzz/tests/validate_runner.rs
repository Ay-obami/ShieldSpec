use std::{fs, process::Command};

fn invoke(script: &str) -> (bool, String) {
    let dir = std::env::temp_dir();
    let fixture = dir.join(format!("shieldspec-validator-{}.json", std::process::id()));
    fs::write(&fixture, r#"{"kind":"orchard_payment","params":{}}"#).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_shieldspec-validate-runner"))
        .args(["/bin/sh", fixture.to_str().unwrap(), "-c", script])
        .output()
        .unwrap();
    let _ = fs::remove_file(fixture);
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn validates_legacy_snapshot() {
    let (ok, stderr) = invoke(
        r#"printf '%s\n' '{"status":"ok","snapshot":{"schema_version":1,"network":"testnet","steps":[]}}'"#,
    );
    assert!(ok, "{stderr}");
}

#[test]
fn accepts_observed_policy_rejection() {
    let (ok, stderr) = invoke(
        r#"printf '%s\n' '{"protocol_version":1,"status":"observed","snapshot":{"schema_version":1,"network":"testnet","steps":[]},"wallet_policy":{"policy":"demo","decision":"rejected"}}'"#,
    );
    assert!(ok, "{stderr}");
}

#[test]
fn rejects_non_json_output() {
    let (ok, stderr) = invoke("printf 'not json'");
    assert!(!ok);
    assert!(stderr.contains("invalid_runner_json"), "{stderr}");
}

#[test]
fn rejects_empty_policy() {
    let (ok, stderr) = invoke(
        r#"printf '%s\n' '{"status":"observed","snapshot":{"schema_version":1,"network":"testnet","steps":[]},"wallet_policy":{"policy":" ","decision":"rejected"}}'"#,
    );
    assert!(!ok);
    assert!(stderr.contains("empty_wallet_policy"), "{stderr}");
}

#[test]
fn rejects_nonzero_exit_even_with_ok_payload() {
    let (ok, stderr) = invoke(
        r#"printf '%s\n' '{"status":"ok","snapshot":{"schema_version":1,"network":"testnet","steps":[]}}'; exit 7"#,
    );
    assert!(!ok);
    assert!(stderr.contains("exit code"), "{stderr}");
}

#[test]
fn rejects_empty_stdout() {
    let (ok, stderr) = invoke("true");
    assert!(!ok);
    assert!(stderr.contains("invalid_runner_json"), "{stderr}");
}
