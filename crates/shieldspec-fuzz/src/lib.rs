//! SDK-independent differential fuzz orchestration.
//!
//! Runner processes exchange only JSON scenarios and normalized ShieldSpec
//! snapshots. No librustzcash Rust type crosses the process boundary.

use std::{
    error::Error,
    fmt,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use shieldspec_core::{assess, compare, Assessment, Policy, ProposalSnapshot, RegressionReport};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RunnerResponse {
    Ok { snapshot: ProposalSnapshot },
    Error { message: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunnerSide {
    Baseline,
    Candidate,
}

impl fmt::Display for RunnerSide {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Baseline => formatter.write_str("baseline"),
            Self::Candidate => formatter.write_str("candidate"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunnerCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl RunnerCommand {
    #[must_use]
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
        }
    }

    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
}

#[derive(Debug)]
pub enum DifferentialError {
    Spawn {
        side: RunnerSide,
        message: String,
    },
    Protocol {
        side: RunnerSide,
        message: String,
    },
    Operational {
        side: RunnerSide,
        exit_code: Option<i32>,
        message: String,
    },
}

impl DifferentialError {
    #[must_use]
    pub const fn side(&self) -> RunnerSide {
        match self {
            Self::Spawn { side, .. }
            | Self::Protocol { side, .. }
            | Self::Operational { side, .. } => *side,
        }
    }
}

impl fmt::Display for DifferentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { side, message } => {
                write!(formatter, "{side} runner could not start: {message}")
            }
            Self::Protocol { side, message } => {
                write!(
                    formatter,
                    "{side} runner violated the JSON protocol: {message}"
                )
            }
            Self::Operational {
                side,
                exit_code,
                message,
            } => {
                write!(
                    formatter,
                    "{side} runner failed operationally (exit {:?}): {message}",
                    exit_code
                )
            }
        }
    }
}

impl Error for DifferentialError {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DifferentialReport {
    pub baseline: Assessment,
    pub candidate: Assessment,
    pub regression: RegressionReport,
}

pub fn run_differential(
    baseline: &RunnerCommand,
    candidate: &RunnerCommand,
    scenario: &Value,
    policy: &Policy,
) -> Result<DifferentialReport, DifferentialError> {
    let baseline_snapshot = run_runner(RunnerSide::Baseline, baseline, scenario)?;
    let candidate_snapshot = run_runner(RunnerSide::Candidate, candidate, scenario)?;

    Ok(DifferentialReport {
        baseline: assess(&baseline_snapshot, policy),
        candidate: assess(&candidate_snapshot, policy),
        regression: compare(&baseline_snapshot, &candidate_snapshot, policy),
    })
}

pub fn compare_snapshots(
    baseline: &ProposalSnapshot,
    candidate: &ProposalSnapshot,
    policy: &Policy,
) -> RegressionReport {
    compare(baseline, candidate, policy)
}

fn run_runner(
    side: RunnerSide,
    runner: &RunnerCommand,
    scenario: &Value,
) -> Result<ProposalSnapshot, DifferentialError> {
    let mut child = Command::new(&runner.program)
        .args(&runner.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| DifferentialError::Spawn {
            side,
            message: error.to_string(),
        })?;

    if let Some(stdin) = child.stdin.as_mut() {
        serde_json::to_writer(&mut *stdin, scenario).map_err(|error| {
            DifferentialError::Protocol {
                side,
                message: format!("could not encode scenario: {error}"),
            }
        })?;
        stdin
            .write_all(b"\n")
            .map_err(|error| DifferentialError::Protocol {
                side,
                message: format!("could not write scenario: {error}"),
            })?;
    }

    let output = child
        .wait_with_output()
        .map_err(|error| DifferentialError::Spawn {
            side,
            message: error.to_string(),
        })?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let response: RunnerResponse =
        serde_json::from_slice(&output.stdout).map_err(|error| DifferentialError::Protocol {
            side,
            message: if stderr.is_empty() {
                error.to_string()
            } else {
                format!("{error}; stderr: {stderr}")
            },
        })?;

    match response {
        RunnerResponse::Ok { snapshot } if output.status.success() => Ok(snapshot),
        RunnerResponse::Ok { .. } => Err(DifferentialError::Operational {
            side,
            exit_code: output.status.code(),
            message: if stderr.is_empty() {
                "runner returned an OK payload with a failing exit status".to_owned()
            } else {
                stderr
            },
        }),
        RunnerResponse::Error { message } => Err(DifferentialError::Operational {
            side,
            exit_code: output.status.code(),
            message,
        }),
    }
}

#[must_use]
pub fn command_exists(path: impl AsRef<Path>) -> bool {
    path.as_ref().is_file()
}
