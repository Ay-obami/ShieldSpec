use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_shieldspec"))
        .current_dir(repository_root())
        .args(args)
        .output()
        .expect("run shieldspec")
}

fn exit_code(output: &Output) -> i32 {
    output
        .status
        .code()
        .expect("process did not exit by signal")
}

fn encoded_file_uri(path: &Path) -> String {
    let path = path
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    let mut encoded = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/' | b':') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(encoded, "%{byte:02X}").unwrap();
        }
    }

    if encoded.starts_with("//") {
        format!("file:{encoded}")
    } else if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

#[test]
fn representative_reports_match_committed_golden_files() {
    for (format, golden) in [
        (
            "human",
            &include_bytes!("golden/transparent-change.human")[..],
        ),
        (
            "json",
            &include_bytes!("golden/transparent-change.json")[..],
        ),
        (
            "sarif",
            &include_bytes!("golden/transparent-change.sarif")[..],
        ),
    ] {
        let output = run(&[
            "check",
            "--input",
            "fixtures/transparent-change.json",
            "--policy",
            "standard",
            "--format",
            format,
        ]);

        assert_eq!(exit_code(&output), 2, "format {format}");
        assert!(output.stderr.is_empty(), "format {format}");
        assert_eq!(output.stdout, golden, "format {format}");
    }
}

#[test]
fn operational_error_matches_committed_golden_file() {
    let output = run(&[
        "check",
        "--input",
        "crates/shieldspec-cli/tests/data/malformed.json",
        "--policy",
        "standard",
        "--format",
        "json",
    ]);

    assert_eq!(exit_code(&output), 1);
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        include_bytes!("golden/malformed-input.stderr")
    );
}

#[test]
fn sarif_encodes_external_special_character_paths_and_uses_static_rule_metadata() {
    let temp_dir = std::env::temp_dir().join(format!(
        "shieldspec SARIF #% {}-{}",
        std::process::id(),
        NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&temp_dir).expect("create temporary directory");
    let input = temp_dir.join("unknown route #%.json");
    fs::write(
        &input,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "network": "test",
            "steps": [{
                "index": 0,
                "inputs": [{ "pool": "unknown", "value_zat": 110000 }],
                "outputs": [{
                    "pool": "unknown",
                    "role": "recipient",
                    "value_zat": 100000
                }],
                "fee_zat": 10000
            }]
        }))
        .unwrap(),
    )
    .expect("write input");

    let output = run(&[
        "check",
        "--input",
        input.to_str().expect("UTF-8 temporary path"),
        "--policy",
        "strict",
        "--format",
        "sarif",
    ]);
    fs::remove_dir_all(&temp_dir).expect("remove temporary directory");

    assert_eq!(exit_code(&output), 2);
    assert!(output.stderr.is_empty());
    let sarif: Value = serde_json::from_slice(&output.stdout).expect("parse SARIF");
    let run = &sarif["runs"][0];
    let rules = run["tool"]["driver"]["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["id"], "SS004_UNKNOWN_ROUTE");
    assert_eq!(
        rules[0]["shortDescription"]["text"],
        "Proposal route has an unknown pool"
    );
    assert_eq!(rules[0]["defaultConfiguration"]["level"], "warning");

    let results = run["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|result| result["level"] == "error"));
    assert!(results.iter().all(|result| {
        result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
            == encoded_file_uri(&input)
    }));
    let messages: BTreeSet<_> = results
        .iter()
        .map(|result| result["message"]["text"].as_str().unwrap())
        .collect();
    assert_eq!(
        messages,
        BTreeSet::from([
            "Input route has an unknown pool",
            "Output route has an unknown pool"
        ])
    );
    let uri = results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
        .as_str()
        .unwrap();
    assert!(uri.contains("%20") && uri.contains("%23") && uri.contains("%25"));
}
