//! SDK-independent differential fuzz orchestration.
//!
//! Runner processes exchange only JSON scenarios and normalized ShieldSpec
//! snapshots. No librustzcash Rust type crosses the process boundary.

use std::{
    error::Error,
    fmt,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use shieldspec_core::{assess, compare, Assessment, Policy, ProposalSnapshot, RegressionReport};

/// A policy outcome reported by the wallet adapter, not inferred from a snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletPolicyDecision {
    Accepted,
    Rejected,
}

/// The runner must invoke the named wallet policy before emitting this observation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WalletPolicyVerdict {
    pub policy: String,
    pub decision: WalletPolicyDecision,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RunnerResponse {
    Ok {
        snapshot: ProposalSnapshot,
    },
    /// The wallet made a real policy decision about the proposed transaction.
    /// A snapshot is required so independent ShieldSpec checks remain possible.
    Observed {
        snapshot: ProposalSnapshot,
        wallet_policy: WalletPolicyVerdict,
    },
    Error {
        message: String,
    },
}

/// Current explicitly versioned runner JSON protocol. Unversioned responses remain
/// valid legacy wire messages for compatibility with existing runner binaries.
pub const RUNNER_PROTOCOL_VERSION: u64 = 1;

/// A malformed runner response is distinct from a valid operational failure.
/// Codes are stable identifiers for callers that need machine-readable diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RunnerProtocolViolation {
    InvalidJson(String),
    InvalidVersion,
    UnsupportedVersion { found: u64, supported: u64 },
    EmptyWalletPolicy,
}

impl RunnerProtocolViolation {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidJson(_) => "invalid_runner_json",
            Self::InvalidVersion => "invalid_runner_protocol_version",
            Self::UnsupportedVersion { .. } => "unsupported_runner_protocol_version",
            Self::EmptyWalletPolicy => "empty_wallet_policy",
        }
    }
}

impl fmt::Display for RunnerProtocolViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(formatter, "invalid runner JSON: {error}"),
            Self::InvalidVersion => {
                formatter.write_str("protocol_version must be a JSON unsigned integer")
            }
            Self::UnsupportedVersion { found, supported } => {
                write!(
                    formatter,
                    "unsupported runner protocol version {found}; supported version is {supported}"
                )
            }
            Self::EmptyWalletPolicy => {
                formatter.write_str("observed wallet policy name must not be empty")
            }
        }
    }
}

impl Error for RunnerProtocolViolation {}

/// Parse either a legacy unversioned response or an explicitly versioned v1
/// response. The version is an optional envelope field, deliberately outside
/// `RunnerResponse` so existing Rust constructors and serializers keep working.
pub fn parse_runner_response(wire: &[u8]) -> Result<RunnerResponse, RunnerProtocolViolation> {
    let value: Value = serde_json::from_slice(wire)
        .map_err(|error| RunnerProtocolViolation::InvalidJson(error.to_string()))?;

    if let Some(version) = value.get("protocol_version") {
        let found = version
            .as_u64()
            .ok_or(RunnerProtocolViolation::InvalidVersion)?;
        if found != RUNNER_PROTOCOL_VERSION {
            return Err(RunnerProtocolViolation::UnsupportedVersion {
                found,
                supported: RUNNER_PROTOCOL_VERSION,
            });
        }
    }

    let response: RunnerResponse = serde_json::from_value(value)
        .map_err(|error| RunnerProtocolViolation::InvalidJson(error.to_string()))?;
    if let RunnerResponse::Observed { wallet_policy, .. } = &response {
        if wallet_policy.policy.trim().is_empty() {
            return Err(RunnerProtocolViolation::EmptyWalletPolicy);
        }
    }
    Ok(response)
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

/// Reproduction identity for the *actual executable bytes* run during discovery.
/// `declared_sdk_revision` is user-supplied context, not cryptographic proof of
/// the source revision; SHA-256 identifies the binary that was executed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerProvenance {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_sdk_revision: Option<String>,
}

impl RunnerProvenance {
    /// Capture an exact executable fingerprint before fuzzing or saving a case.
    pub fn capture(
        command: &RunnerCommand,
        declared_sdk_revision: Option<String>,
    ) -> Result<Self, String> {
        let program = std::fs::canonicalize(&command.program).map_err(|error| {
            format!(
                "could not locate runner {}: {error}",
                command.program.display()
            )
        })?;
        let sha256 = hash_executable(&program)?;
        Ok(Self {
            program,
            args: command.args.clone(),
            sha256,
            declared_sdk_revision,
        })
    }

    /// Refuse to replay silently with different binary bytes or arguments.
    /// Paths may change when the identical runner binary moves machines.
    pub fn verify(&self, command: &RunnerCommand) -> Result<(), String> {
        let actual_path = std::fs::canonicalize(&command.program).map_err(|error| {
            format!(
                "runner path {} could not be verified: {error}",
                command.program.display()
            )
        })?;
        if command.args != self.args {
            return Err("runner arguments differ from recorded discovery".to_owned());
        }
        let actual_sha256 = hash_executable(&actual_path)?;
        if actual_sha256 != self.sha256 {
            return Err(format!(
                "runner SHA-256 mismatch for {}: recorded {}, actual {}",
                actual_path.display(),
                self.sha256,
                actual_sha256
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn command(&self) -> RunnerCommand {
        RunnerCommand {
            program: self.program.clone(),
            args: self.args.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DifferentialProvenance {
    pub baseline: RunnerProvenance,
    pub candidate: RunnerProvenance,
    pub policy: String,
}

fn hash_executable(path: &Path) -> Result<String, String> {
    let mut executable = File::open(path)
        .map_err(|error| format!("could not open runner {}: {error}", path.display()))?;
    let mut sha256 = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = executable
            .read(&mut buffer)
            .map_err(|error| format!("could not read runner {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        sha256.update(&buffer[..count]);
    }
    Ok(format!("{:x}", sha256.finalize()))
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

/// Why a differential report cannot support a comparable wallet-policy claim.
/// Snapshot-only comparisons still succeed; callers explicitly opt into this check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingWalletPolicyObservations {
    Baseline,
    Candidate,
    Both,
}

impl MissingWalletPolicyObservations {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        "missing_wallet_policy_observations"
    }
}

impl fmt::Display for MissingWalletPolicyObservations {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Baseline => {
                formatter.write_str("baseline runner provided no wallet-policy observation")
            }
            Self::Candidate => {
                formatter.write_str("candidate runner provided no wallet-policy observation")
            }
            Self::Both => {
                formatter.write_str("neither runner provided a wallet-policy observation")
            }
        }
    }
}

impl Error for MissingWalletPolicyObservations {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WalletPolicyDifference {
    pub policy: String,
    pub baseline: WalletPolicyDecision,
    pub candidate: WalletPolicyDecision,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DifferentialReport {
    pub baseline: Assessment,
    pub candidate: Assessment,
    pub regression: RegressionReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_policy_difference: Option<WalletPolicyDifference>,
    /// Raw, independently observed outcomes. Their absence is not a passing verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_wallet_policy: Option<WalletPolicyVerdict>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_wallet_policy: Option<WalletPolicyVerdict>,
}

impl DifferentialReport {
    /// A wallet decision difference is independent of ShieldSpec snapshot regressions.
    #[must_use]
    pub fn has_difference(&self) -> bool {
        !self.regression.passes || self.wallet_policy_difference.is_some()
    }

    /// Require evidence from both wallets before asserting observed agreement or
    /// difference. Legacy snapshot-only runs remain valid without this opt-in check.
    pub fn require_comparable_wallet_policy_observations(
        &self,
    ) -> Result<(), MissingWalletPolicyObservations> {
        match (&self.baseline_wallet_policy, &self.candidate_wallet_policy) {
            (Some(_), Some(_)) => Ok(()),
            (None, Some(_)) => Err(MissingWalletPolicyObservations::Baseline),
            (Some(_), None) => Err(MissingWalletPolicyObservations::Candidate),
            (None, None) => Err(MissingWalletPolicyObservations::Both),
        }
    }

    /// The shrinker must retain the primary original discovery, not merely any finding.
    #[must_use]
    pub fn preserves_discovery(&self, original: &Self) -> bool {
        if let Some(expected) = &original.wallet_policy_difference {
            return self.wallet_policy_difference.as_ref() == Some(expected);
        }
        original
            .regression
            .findings
            .first()
            .is_some_and(|expected| {
                self.regression
                    .findings
                    .iter()
                    .any(|finding| finding.rule_id == expected.rule_id)
            })
    }
}

struct RunnerObservation {
    snapshot: ProposalSnapshot,
    wallet_policy: Option<WalletPolicyVerdict>,
}

pub fn run_differential(
    baseline: &RunnerCommand,
    candidate: &RunnerCommand,
    scenario: &Value,
    policy: &Policy,
) -> Result<DifferentialReport, DifferentialError> {
    let baseline_result = run_runner(RunnerSide::Baseline, baseline, scenario)?;
    let candidate_result = run_runner(RunnerSide::Candidate, candidate, scenario)?;

    if let (Some(before), Some(after)) = (
        &baseline_result.wallet_policy,
        &candidate_result.wallet_policy,
    ) {
        if before.policy != after.policy {
            return Err(DifferentialError::Protocol {
                side: RunnerSide::Candidate,
                message: format!(
                    "wallet policy mismatch: baseline {}, candidate {}",
                    before.policy, after.policy
                ),
            });
        }
    }

    // Compare only decisions explicitly observed under the same named wallet policy.
    // A legacy runner that emits only a snapshot does not provide policy evidence.
    let wallet_policy_difference = match (
        &baseline_result.wallet_policy,
        &candidate_result.wallet_policy,
    ) {
        (Some(before), Some(after))
            if before.policy == after.policy && before.decision != after.decision =>
        {
            Some(WalletPolicyDifference {
                policy: before.policy.clone(),
                baseline: before.decision,
                candidate: after.decision,
            })
        }
        _ => None,
    };

    Ok(DifferentialReport {
        baseline: assess(&baseline_result.snapshot, policy),
        candidate: assess(&candidate_result.snapshot, policy),
        regression: compare(
            &baseline_result.snapshot,
            &candidate_result.snapshot,
            policy,
        ),
        wallet_policy_difference,
        baseline_wallet_policy: baseline_result.wallet_policy,
        candidate_wallet_policy: candidate_result.wallet_policy,
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
) -> Result<RunnerObservation, DifferentialError> {
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
    let response =
        parse_runner_response(&output.stdout).map_err(|error| DifferentialError::Protocol {
            side,
            message: if stderr.is_empty() {
                format!("{}: {error}", error.code())
            } else {
                format!("{}: {error}; stderr: {stderr}", error.code())
            },
        })?;

    match response {
        RunnerResponse::Ok { snapshot } if output.status.success() => Ok(RunnerObservation {
            snapshot,
            wallet_policy: None,
        }),
        RunnerResponse::Observed {
            snapshot,
            wallet_policy,
        } if output.status.success() => Ok(RunnerObservation {
            snapshot,
            wallet_policy: Some(wallet_policy),
        }),
        RunnerResponse::Ok { .. } | RunnerResponse::Observed { .. } => {
            Err(DifferentialError::Operational {
                side,
                exit_code: output.status.code(),
                message: if stderr.is_empty() {
                    "runner supplied an invalid observation or a failing exit status".to_owned()
                } else {
                    stderr
                },
            })
        }
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
