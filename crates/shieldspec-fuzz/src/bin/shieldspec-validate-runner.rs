//! Structural preflight only: verifies one runner's process/JSON contract, not privacy.
use shieldspec_fuzz::{parse_runner_response, RunnerResponse};
use std::{
    env, fs,
    io::Write,
    process::{Command, ExitCode, Stdio},
};

fn validate() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let executable = args
        .next()
        .ok_or("usage: shieldspec-validate-runner <runner> <scenario.json> [runner-args...]")?;
    let scenario_path = args.next().ok_or("missing scenario JSON path")?;
    let input = fs::read(&scenario_path).map_err(|e| format!("cannot read scenario: {e}"))?;
    serde_json::from_slice::<serde_json::Value>(&input)
        .map_err(|e| format!("invalid scenario JSON: {e}"))?;
    let mut child = Command::new(executable)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot launch runner: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("runner stdin unavailable")?
        .write_all(&input)
        .map_err(|e| format!("cannot write scenario to runner: {e}"))?;
    let result = child
        .wait_with_output()
        .map_err(|e| format!("runner wait failed: {e}"))?;
    let response =
        parse_runner_response(&result.stdout).map_err(|e| format!("{}: {e}", e.code()))?;
    if !result.status.success() {
        return Err(format!(
            "runner operational failure (exit code {:?}): {response:?}",
            result.status.code()
        ));
    }
    if let RunnerResponse::Error { message } = response {
        return Err(format!(
            "runner reported operational error with exit 0: {message}"
        ));
    }
    println!("Runner protocol preflight passed (structural check only; no privacy claim)");
    Ok(())
}

fn main() -> ExitCode {
    match validate() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Runner protocol preflight failed: {error}");
            ExitCode::FAILURE
        }
    }
}
