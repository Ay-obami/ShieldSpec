use std::{fs, path::PathBuf};

use shieldspec_fuzz::{RunnerCommand, RunnerProvenance};

#[test]
fn same_runner_bytes_capture_an_identical_sha256_and_declared_sdk_revision() {
    let executable = std::env::current_exe().expect("test executable");
    let command = RunnerCommand::new(&executable).arg("runner");
    let first = RunnerProvenance::capture(&command, Some("0.25.0-pre.2".to_owned()))
        .expect("fingerprint running executable");
    let second = RunnerProvenance::capture(&command, Some("0.25.0-pre.2".to_owned()))
        .expect("re-fingerprint running executable");

    assert_eq!(first, second);
    assert_eq!(first.sha256.len(), 64);
    assert_eq!(first.declared_sdk_revision.as_deref(), Some("0.25.0-pre.2"));
    assert_eq!(first.args, vec!["runner"]);
    first.verify(&command).expect("unchanged executable");
}

#[test]
fn changed_runner_contents_fail_closed_on_replay() {
    let test_file = std::env::temp_dir().join(format!(
        "shieldspec-runner-provenance-test-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("thread")
    ));
    fs::write(&test_file, b"runner-original").expect("write test runner bytes");
    let command = RunnerCommand::new(&test_file);
    let original = RunnerProvenance::capture(&command, None).expect("capture original bytes");

    fs::write(&test_file, b"runner-modified").expect("modify runner bytes");
    let failure = original
        .verify(&command)
        .expect_err("modified bytes must fail");
    assert!(failure.to_string().contains("SHA-256"), "{failure}");

    fs::remove_file(test_file).expect("clean up own test file");
}

#[test]
fn a_missing_runner_cannot_substitute_for_recorded_binary() {
    let command = RunnerCommand::new(std::env::current_exe().expect("current executable"));
    let provenance = RunnerProvenance::capture(&command, None).expect("fingerprint");
    let fake = RunnerCommand::new(PathBuf::from("/tmp/not-the-recorded-runner"));
    let failure = provenance
        .verify(&fake)
        .expect_err("path mismatch must fail");
    assert!(failure.to_string().contains("runner path"), "{failure}");
}
