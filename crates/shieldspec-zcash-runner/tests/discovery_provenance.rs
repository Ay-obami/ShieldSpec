use std::{fs, process::Command};

use serde_json::Value;
use shieldspec_zcash_runner::Discovery;

#[test]
fn old_corpus_cases_still_decode_without_provenance() {
    let discovery: Discovery = serde_json::from_str(include_str!(
        "../fixtures/discoveries/tex-transparent-recipient.json"
    ))
    .expect("legacy corpus remains valid");
    assert!(discovery.provenance.is_none());
}

#[test]
fn discovered_fault_records_binary_hashes_and_replays_automatically() {
    let binary = env!("CARGO_BIN_EXE_shieldspec-fuzz");
    let file = std::env::temp_dir().join(format!(
        "shieldspec-provenance-discovery-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));

    let fuzz = Command::new(binary)
        .args([
            "fuzz",
            "--baseline-runner",
            binary,
            "--baseline-arg",
            "runner",
            "--candidate-runner",
            binary,
            "--candidate-arg",
            "runner",
            "--candidate-arg",
            "--demo-regression",
            "--candidate-arg",
            "transparent-change",
            "--baseline-sdk",
            "zcash_client_backend=0.25.0-pre.2",
            "--candidate-sdk",
            "zcash_client_backend=0.25.0-pre.2",
            "--seed",
            "148",
            "--cases",
            "1",
            "--save",
            file.to_str().expect("temporary path"),
            "--format",
            "json",
        ])
        .output()
        .expect("start real-fuzzer demo fault");
    assert_eq!(
        fuzz.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&fuzz.stderr)
    );

    let json = fs::read(&file).expect("written discovery");
    let discovery: Value = serde_json::from_slice(&json).expect("valid saved JSON");
    assert_eq!(
        discovery["provenance"]["baseline"]["sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(
        discovery["provenance"]["candidate"]["sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(discovery["provenance"]["policy"], "standard");
    assert_eq!(
        discovery["provenance"]["baseline"]["declared_sdk_revision"],
        "zcash_client_backend=0.25.0-pre.2"
    );

    let replay = Command::new(binary)
        .args([
            "replay",
            "--input",
            file.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("replay saved binary identities");
    assert_eq!(
        replay.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    let replay_json: Value = serde_json::from_slice(&replay.stdout).expect("replay JSON");
    assert_eq!(replay_json["recorded_rule_reproduced"], true);

    let mut tampered = discovery;
    tampered["provenance"]["candidate"]["sha256"] = Value::String("0".repeat(64));
    fs::write(&file, serde_json::to_vec(&tampered).unwrap()).expect("mutate only test manifest");
    let rejected = Command::new(binary)
        .args(["replay", "--input", file.to_str().unwrap()])
        .output()
        .expect("start tampered replay");
    assert_eq!(rejected.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("SHA-256 mismatch"));

    fs::remove_file(file).expect("delete test-specific discovery");
}
