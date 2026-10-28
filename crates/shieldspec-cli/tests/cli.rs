use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

fn fixture(name: &str) -> PathBuf {
    let path = temp_file(&format!("fixture-{name}.json"));
    shieldspec_testkit::write_fixture(name, &path)
        .unwrap_or_else(|error| panic!("could not materialize fixture {name}: {error}"));
    path
}

fn temp_file(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "shieldspec-cli-{label}-{}-{}",
        std::process::id(),
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    ))
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_shieldspec"))
        .args(args)
        .output()
        .expect("run shieldspec")
}

fn code(output: &Output) -> i32 {
    output
        .status
        .code()
        .expect("process did not exit by signal")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn check(path: &Path, policy: &str, format: &str) -> Output {
    run(&[
        "check",
        "--input",
        path.to_str().expect("UTF-8 path"),
        "--policy",
        policy,
        "--format",
        format,
    ])
}

fn diff(baseline: &Path, candidate: &Path, format: &str) -> Output {
    run(&[
        "diff",
        "--baseline",
        baseline.to_str().expect("UTF-8 path"),
        "--candidate",
        candidate.to_str().expect("UTF-8 path"),
        "--policy",
        "standard",
        "--format",
        format,
    ])
}

#[test]
fn check_uses_the_locked_pass_and_privacy_failure_exit_codes() {
    let passing = check(&fixture("shielded-payment"), "standard", "human");
    assert_eq!(code(&passing), 0, "{}", stdout(&passing));
    assert!(stdout(&passing).contains("PASS"));

    let failing = check(&fixture("transparent-change"), "standard", "human");
    assert_eq!(code(&failing), 2, "{}", stdout(&failing));
    assert!(stdout(&failing).contains("SS003_TRANSPARENT_CHANGE"));
}

#[test]
fn diff_passes_without_a_regression_and_fails_for_a_new_finding() {
    let baseline = fixture("shielded-payment");
    let same = diff(&baseline, &baseline, "json");
    assert_eq!(code(&same), 0, "{}", stdout(&same));
    assert_eq!(
        serde_json::from_slice::<Value>(&same.stdout).unwrap()["passes"],
        true
    );

    let regression = diff(&baseline, &fixture("transparent-change"), "json");
    assert_eq!(code(&regression), 2, "{}", stdout(&regression));
    let report: Value = serde_json::from_slice(&regression.stdout).unwrap();
    assert_eq!(report["passes"], false);
    assert!(report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| { finding["rule_id"] == "SS003_TRANSPARENT_CHANGE" }));
}

#[test]
fn malformed_unsupported_and_unreadable_inputs_are_operational_errors() {
    let malformed = temp_file("malformed.json");
    fs::write(&malformed, b"{not json").unwrap();
    assert_eq!(code(&check(&malformed, "standard", "json")), 1);
    fs::remove_file(&malformed).unwrap();

    let unsupported_schema = temp_file("schema.json");
    fs::write(
        &unsupported_schema,
        serde_json::to_vec(&json!({
            "schema_version": 99,
            "network": "test",
            "steps": []
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(code(&check(&unsupported_schema, "standard", "json")), 1);
    fs::remove_file(&unsupported_schema).unwrap();

    assert_eq!(
        code(&check(Path::new("does-not-exist.json"), "standard", "json")),
        1
    );
    assert_eq!(
        code(&check(&fixture("shielded-payment"), "lenient", "json")),
        1
    );
    assert_eq!(
        code(&check(&fixture("shielded-payment"), "standard", "xml")),
        1
    );
}

#[test]
fn strict_policy_is_accepted_and_upgrades_warning_severity() {
    let output = check(&fixture("transparent-recipient"), "strict", "json");
    assert_eq!(code(&output), 2);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|finding| finding["severity"] == "error"));
}

#[test]
fn fixture_list_and_write_expose_the_canonical_fixtures() {
    let listed = run(&["fixture", "list"]);
    assert_eq!(code(&listed), 0);
    assert_eq!(
        stdout(&listed).lines().collect::<Vec<_>>(),
        vec![
            "shielded-payment",
            "transparent-recipient",
            "transparent-funding",
            "transparent-change",
            "unified-selected-transparent",
            "multi-step-late-leak",
            "privacy-preserving-migration",
            "full-balance-migration",
        ]
    );

    let destination = temp_file("fixture.json");
    let written = run(&[
        "fixture",
        "write",
        "shielded-payment",
        destination.to_str().unwrap(),
    ]);
    assert_eq!(code(&written), 0);
    let actual: Value = serde_json::from_slice(&fs::read(&destination).unwrap()).unwrap();
    let expected: Value =
        serde_json::from_slice(&fs::read(fixture("shielded-payment")).unwrap()).unwrap();
    assert_eq!(actual, expected);
    fs::remove_file(destination).unwrap();

    assert_eq!(
        code(&run(&["fixture", "write", "not-a-fixture", "unused.json"])),
        1
    );
}

#[test]
fn human_json_and_sarif_describe_identical_findings() {
    let input = fixture("transparent-change");
    let human = check(&input, "standard", "human");
    let json_output = check(&input, "standard", "json");
    let sarif_output = check(&input, "standard", "sarif");
    assert_eq!(
        (code(&human), code(&json_output), code(&sarif_output)),
        (2, 2, 2)
    );

    let report: Value = serde_json::from_slice(&json_output.stdout).unwrap();
    let sarif: Value = serde_json::from_slice(&sarif_output.stdout).unwrap();
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(
        sarif["$schema"],
        "https://json.schemastore.org/sarif-2.1.0.json"
    );

    let results = sarif["runs"][0]["results"].as_array().unwrap();
    let findings = report["findings"].as_array().unwrap();
    assert_eq!(results.len(), findings.len());
    let human = stdout(&human);

    for (finding, result) in findings.iter().zip(results) {
        assert_eq!(result["ruleId"], finding["rule_id"]);
        assert_eq!(result["level"], finding["severity"]);
        assert_eq!(result["message"]["text"], finding["message"]);
        assert_eq!(result["properties"]["step_index"], finding["step_index"]);
        assert_eq!(result["properties"]["severity"], finding["severity"]);
        assert_eq!(result["properties"]["evidence"], finding["evidence"]);
        assert_eq!(result["properties"]["fingerprint"], finding["fingerprint"]);
        assert_eq!(
            result["partialFingerprints"]["shieldspecFingerprint"],
            finding["fingerprint"]
        );

        for key in ["rule_id", "severity", "message", "fingerprint"] {
            assert!(
                human.contains(finding[key].as_str().unwrap()),
                "missing {key}"
            );
        }
        assert!(human.contains(&format!("step {}", finding["step_index"])));
        let evidence: BTreeMap<String, String> =
            serde_json::from_value(finding["evidence"].clone()).unwrap();
        for (key, value) in evidence {
            assert!(human.contains(&key));
            assert!(human.contains(&value));
        }
    }

    let rules = sarif["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .unwrap();
    let rule_ids: Vec<_> = rules.iter().map(|rule| rule["id"].clone()).collect();
    let mut sorted = rule_ids.clone();
    sorted.sort_by_key(Value::to_string);
    sorted.dedup();
    assert_eq!(rule_ids, sorted, "SARIF rules must be sorted and unique");
}

#[test]
fn repeated_output_is_byte_for_byte_deterministic() {
    let input = fixture("multi-step-late-leak");
    for format in ["human", "json", "sarif"] {
        let first = check(&input, "standard", format);
        let second = check(&input, "standard", format);
        assert_eq!(first.stdout, second.stdout, "format {format}");
    }
}

fn write_json(label: &str, value: Value) -> PathBuf {
    let path = temp_file(label);
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    path
}

#[test]
fn invalid_snapshots_are_operational_errors() {
    let cases = [
        (
            "empty",
            json!({"schema_version": 1, "network": "testnet", "steps": []}),
        ),
        (
            "unknown-field",
            json!({
                "schema_version": 1,
                "network": "testnet",
                "steps": [],
                "surprise": true
            }),
        ),
        (
            "bad-indexes",
            json!({
                "schema_version": 1,
                "network": "testnet",
                "steps": [{
                    "index": 1,
                    "inputs": [{"pool": "orchard", "value_zat": 10}],
                    "outputs": [{"pool": "orchard", "role": "other", "value_zat": 9}],
                    "fee_zat": 1
                }]
            }),
        ),
        (
            "unbalanced",
            json!({
                "schema_version": 1,
                "network": "testnet",
                "steps": [{
                    "index": 0,
                    "inputs": [{"pool": "orchard", "value_zat": 10}],
                    "outputs": [{"pool": "orchard", "role": "other", "value_zat": 8}],
                    "fee_zat": 1
                }]
            }),
        ),
    ];

    for (label, value) in cases {
        let path = write_json(label, value);
        let output = check(&path, "standard", "json");
        assert_eq!(code(&output), 1, "{label}: {}", stdout(&output));
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn cross_network_diff_is_an_operational_error() {
    let baseline = fixture("shielded-payment");
    let mut candidate: Value = serde_json::from_slice(&fs::read(&baseline).unwrap()).unwrap();
    candidate["network"] = json!("mainnet");
    let candidate = write_json("cross-network.json", candidate);

    let output = diff(&baseline, &candidate, "json");
    assert_eq!(code(&output), 1, "{}", stdout(&output));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("network"));
    fs::remove_file(candidate).unwrap();
}

#[test]
fn try_runs_a_safe_builtin_control_and_explains_its_scope() {
    let output = run(&["try"]);
    assert_eq!(code(&output), 0, "{}", stdout(&output));
    assert!(stdout(&output).contains("built-in snapshot"));
    assert!(stdout(&output).contains("not your wallet"));
}

#[test]
fn test_reads_a_project_config_and_preserves_failure_exit_codes() {
    let baseline = fixture("shielded-payment");
    let candidate = fixture("transparent-change");
    let config = temp_file("project.toml");
    fs::write(
        &config,
        format!(
            "[comparison]\nbaseline = {:?}\ncandidate = {:?}\n[testing]\npolicy = \"standard\"\n",
            baseline.to_str().unwrap(),
            candidate.to_str().unwrap()
        ),
    )
    .unwrap();
    let output = run(&["test", "--config", config.to_str().unwrap()]);
    assert_eq!(code(&output), 2, "{}", stdout(&output));
    assert!(stdout(&output).contains("SS003_TRANSPARENT_CHANGE"));
    fs::remove_file(config).unwrap();
}

#[test]
fn test_rejects_missing_config_without_a_success_verdict() {
    let output = run(&["test", "--config", "missing-shieldspec.toml"]);
    assert_eq!(code(&output), 1);
    assert!(output.stdout.is_empty());
}

fn fresh_project_directory(label: &str) -> PathBuf {
    let dir = temp_file(label);
    fs::create_dir(&dir).expect("create test project directory");
    dir
}

fn run_from(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_shieldspec"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run shieldspec in project directory")
}

#[test]
fn init_bootstraps_a_safe_self_control_and_test_runs_without_extra_flags() {
    let dir = fresh_project_directory("init-project");
    let initialized = run_from(&dir, &["init"]);
    assert_eq!(
        code(&initialized),
        0,
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    assert!(
        stdout(&initialized).contains("not your wallet")
            || stdout(&initialized).contains("self-control")
    );
    for filename in ["shieldspec.toml", "baseline.json", "candidate.json"] {
        assert!(dir.join(filename).is_file(), "missing {filename}");
    }
    let config = fs::read_to_string(dir.join("shieldspec.toml")).unwrap();
    assert!(config.contains("baseline.json"));
    assert!(config.contains("candidate.json"));
    let check = run_from(&dir, &["test"]);
    assert_eq!(
        code(&check),
        0,
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(stdout(&check).contains("snapshot"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn init_refuses_to_overwrite_existing_project_data() {
    let dir = fresh_project_directory("init-existing");
    fs::write(dir.join("baseline.json"), "owner data").unwrap();
    let initialized = run_from(&dir, &["init"]);
    assert_eq!(code(&initialized), 1);
    assert_eq!(
        fs::read_to_string(dir.join("baseline.json")).unwrap(),
        "owner data"
    );
    assert!(!dir.join("shieldspec.toml").exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn compare_flag_rejects_snapshot_configuration_instead_of_silently_running_it() {
    let dir = fresh_project_directory("compare-snapshot");
    let initialized = run_from(&dir, &["init"]);
    assert_eq!(code(&initialized), 0);
    let compared = run_from(&dir, &["test", "--compare"]);
    assert_eq!(code(&compared), 1);
    assert!(String::from_utf8_lossy(&compared.stderr).contains("differential"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn compare_flag_preserves_operational_failure_for_missing_differential_fuzzer() {
    let dir = fresh_project_directory("compare-fuzzer");
    fs::write(dir.join("shieldspec.toml"), "[comparison]\nmode = \"differential\"\nbaseline_runner = \"baseline\"\ncandidate_runner = \"candidate\"\nfuzzer_binary = \"nonexistent-shieldspec-fuzz-executable\"\n[testing]\ncases = 2\nseed = 2026\n").unwrap();
    let compared = run_from(&dir, &["test", "--compare"]);
    assert_eq!(code(&compared), 1);
    assert!(String::from_utf8_lossy(&compared.stderr).contains("fuzzer"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn compare_flag_runs_configured_differential_fuzzer_and_forwards_runner_arguments() {
    let dir = fresh_project_directory("compare-forward");
    fs::write(dir.join("shieldspec.toml"),
        "[comparison]\nmode = \"differential\"\nfuzzer_binary = \"/bin/echo\"\nbaseline_runner = \"baseline-wallet\"\ncandidate_runner = \"candidate-wallet\"\nbaseline_args = [\"--wallet-spend-policy\", \"orchard-only\"]\ncandidate_args = [\"--wallet-spend-policy\", \"orchard-only\"]\n[testing]\nscenario_family = \"ironwood\"\nseed = 2026\ncases = 4\n").unwrap();
    let compared = run_from(&dir, &["test", "--compare"]);
    assert_eq!(
        code(&compared),
        0,
        "{}",
        String::from_utf8_lossy(&compared.stderr)
    );
    let text = stdout(&compared);
    assert!(text.contains("fuzz --baseline-runner"), "{text}");
    assert!(text.contains("--candidate-runner"), "{text}");
    assert!(text.contains("--scenario-family ironwood"), "{text}");
    assert!(
        text.contains("--baseline-arg=--wallet-spend-policy"),
        "{text}"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn replay_forwards_the_input_to_the_configured_fuzzer_without_changing_verification_mode() {
    let dir = fresh_project_directory("replay-forward");
    fs::write(
        dir.join("shieldspec.toml"),
        "[comparison]\nmode = \"differential\"\nfuzzer_binary = \"/bin/echo\"\n",
    )
    .unwrap();
    fs::write(dir.join("discovery.json"), "{}").unwrap();
    let replayed = run_from(&dir, &["replay", "discovery.json"]);
    assert_eq!(
        code(&replayed),
        0,
        "{}",
        String::from_utf8_lossy(&replayed.stderr)
    );
    assert!(
        stdout(&replayed).contains("replay --input discovery.json"),
        "{}",
        stdout(&replayed)
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn replay_reports_missing_fuzzer_as_an_operational_error() {
    let dir = fresh_project_directory("replay-missing-fuzzer");
    fs::write(
        dir.join("shieldspec.toml"),
        "[comparison]\nmode = \"differential\"\nfuzzer_binary = \"nonexistent-shieldspec-fuzz-executable\"\n",
    ).unwrap();
    fs::write(dir.join("discovery.json"), "{}").unwrap();
    let replayed = run_from(&dir, &["replay", "discovery.json"]);
    assert_eq!(code(&replayed), 1);
    assert!(String::from_utf8_lossy(&replayed.stderr).contains("fuzzer"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn differential_project_config_reports_missing_fuzzer_as_an_operational_error() {
    let config = temp_file("differential.toml");
    fs::write(&config,
        "[comparison]\nmode = \"differential\"\nbaseline_runner = \"baseline\"\ncandidate_runner = \"candidate\"\nfuzzer_binary = \"nonexistent-shieldspec-fuzz-executable\"\n[testing]\ncases = 2\nseed = 2026\n"
    ).unwrap();
    let output = run(&["test", "--config", config.to_str().unwrap()]);
    assert_eq!(code(&output), 1);
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("fuzzer"));
    fs::remove_file(config).unwrap();
}
