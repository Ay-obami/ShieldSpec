mod reporter;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};

use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use shieldspec_core::{
    assess, compare, validate_comparison_context, validate_snapshot, Finding, Policy,
    ProposalSnapshot,
};
use shieldspec_testkit::{fixture_names, write_fixture};

use crate::reporter::{render, ReportFormat};

#[derive(Debug, Parser)]
#[command(
    name = "shieldspec",
    about = "Check proposal snapshots for privacy regressions"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run a built-in snapshot example (not a test of your wallet).
    Try,
    /// Test a project using its shieldspec.toml configuration.
    Test {
        #[arg(long, default_value = "shieldspec.toml")]
        config: PathBuf,
    },
    /// Check one proposal snapshot against a privacy policy.
    Check {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, value_enum)]
        policy: PolicyName,
        #[arg(long, value_enum)]
        format: FormatName,
    },
    /// Compare a candidate snapshot with a baseline.
    Diff {
        #[arg(long)]
        baseline: PathBuf,
        #[arg(long)]
        candidate: PathBuf,
        #[arg(long, value_enum)]
        policy: PolicyName,
        #[arg(long, value_enum)]
        format: FormatName,
    },
    /// List or write canonical proposal fixtures.
    Fixture {
        #[command(subcommand)]
        command: FixtureCommand,
    },
}

#[derive(Debug, Subcommand)]
enum FixtureCommand {
    /// List canonical fixture names.
    List,
    /// Write a canonical fixture to a path.
    Write { name: String, path: PathBuf },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum PolicyName {
    Standard,
    Strict,
}

impl PolicyName {
    const fn policy(self) -> Policy {
        match self {
            Self::Standard => Policy::standard(),
            Self::Strict => Policy::strict(),
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum FormatName {
    Human,
    Json,
    Sarif,
}

impl From<FormatName> for ReportFormat {
    fn from(value: FormatName) -> Self {
        match value {
            FormatName::Human => Self::Human,
            FormatName::Json => Self::Json,
            FormatName::Sarif => Self::Sarif,
        }
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let informational = matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            );
            let _ = error.print();
            return if informational {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            };
        }
    };

    match execute(cli.command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("shieldspec: {error}");
            ExitCode::from(1)
        }
    }
}

fn execute(command: Command) -> Result<ExitCode, String> {
    match command {
        Command::Try => {
            let snapshot = shieldspec_testkit::fixture("shielded-payment")
                .ok_or_else(|| "built-in fixture unavailable".to_owned())?;
            let result = assess(&snapshot, &Policy::standard());
            println!("ShieldSpec built-in snapshot control");
            println!("status: {}", if result.passes { "PASS" } else { "FAIL" });
            println!("findings: {}", result.findings.len());
            println!("note: this is not your wallet or a real SDK policy comparison.");
            println!("next: create shieldspec.toml and run `shieldspec test`.");
            Ok(privacy_exit(result.passes))
        }
        Command::Test { config } => execute_project_test(&config),
        Command::Check {
            input,
            policy,
            format,
        } => {
            let snapshot = read_snapshot(&input)?;
            let assessment = assess(&snapshot, &policy.policy());
            print_report(assessment.passes, &assessment.findings, format, &input)?;
            Ok(privacy_exit(assessment.passes))
        }
        Command::Diff {
            baseline,
            candidate,
            policy,
            format,
        } => {
            let baseline_snapshot = read_snapshot(&baseline)?;
            let candidate_snapshot = read_snapshot(&candidate)?;
            validate_comparison_context(&baseline_snapshot, &candidate_snapshot)
                .map_err(|error| format!("cannot compare snapshots: {error}"))?;
            let report = compare(&baseline_snapshot, &candidate_snapshot, &policy.policy());
            print_report(report.passes, &report.findings, format, &candidate)?;
            Ok(privacy_exit(report.passes))
        }
        Command::Fixture { command } => execute_fixture(command),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectConfig {
    comparison: ProjectComparison,
    #[serde(default)]
    testing: ProjectTesting,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectComparison {
    #[serde(default)]
    mode: Option<String>,
    baseline: Option<PathBuf>,
    candidate: Option<PathBuf>,
    baseline_runner: Option<PathBuf>,
    candidate_runner: Option<PathBuf>,
    #[serde(default)]
    baseline_args: Vec<String>,
    #[serde(default)]
    candidate_args: Vec<String>,
    fuzzer_binary: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectTesting {
    policy: Option<String>,
    seed: Option<u64>,
    cases: Option<u64>,
    scenario_family: Option<String>,
}

fn execute_project_test(config_path: &Path) -> Result<ExitCode, String> {
    let contents = fs::read_to_string(config_path)
        .map_err(|error| format!("could not read config {}: {error}", config_path.display()))?;
    let config: ProjectConfig = toml::from_str(&contents)
        .map_err(|error| format!("invalid config {}: {error}", config_path.display()))?;
    let policy = match config.testing.policy.as_deref().unwrap_or("standard") {
        "standard" => Policy::standard(),
        "strict" => Policy::strict(),
        other => {
            return Err(format!(
                "unsupported policy '{other}'; choose standard or strict"
            ))
        }
    };
    let base = config_path.parent().unwrap_or_else(|| Path::new("."));
    let resolve = |path: PathBuf| {
        if path.is_absolute() {
            path
        } else {
            base.join(path)
        }
    };
    if config.comparison.mode.as_deref() == Some("differential") {
        return run_configured_fuzzer(config, base);
    }
    if !matches!(config.comparison.mode.as_deref(), None | Some("snapshot")) {
        return Err("comparison.mode must be snapshot or differential".to_owned());
    }
    let baseline = resolve(
        config
            .comparison
            .baseline
            .ok_or_else(|| "comparison.baseline is required for snapshot mode".to_owned())?,
    );
    let candidate = resolve(
        config
            .comparison
            .candidate
            .ok_or_else(|| "comparison.candidate is required for snapshot mode".to_owned())?,
    );
    let baseline_snapshot = read_snapshot(&baseline)?;
    let candidate_snapshot = read_snapshot(&candidate)?;
    validate_comparison_context(&baseline_snapshot, &candidate_snapshot)
        .map_err(|error| format!("cannot compare snapshots: {error}"))?;
    let report = compare(&baseline_snapshot, &candidate_snapshot, &policy);
    println!("ShieldSpec project privacy check");
    println!("baseline: {}", baseline.display());
    println!("candidate: {}", candidate.display());
    print_report(
        report.passes,
        &report.findings,
        FormatName::Human,
        &candidate,
    )?;
    println!("note: snapshot findings do not prove wallet policy decisions or anonymity.");
    Ok(privacy_exit(report.passes))
}

fn run_configured_fuzzer(config: ProjectConfig, base: &Path) -> Result<ExitCode, String> {
    let comparison = config.comparison;
    let baseline = comparison
        .baseline_runner
        .ok_or_else(|| "comparison.baseline_runner is required for differential mode".to_owned())?;
    let candidate = comparison.candidate_runner.ok_or_else(|| {
        "comparison.candidate_runner is required for differential mode".to_owned()
    })?;
    let resolve = |path: PathBuf| {
        if path.is_absolute() {
            path
        } else {
            base.join(path)
        }
    };
    let fuzzer = comparison
        .fuzzer_binary
        .unwrap_or_else(|| PathBuf::from("shieldspec-fuzz"));
    let fuzzer = if fuzzer.components().count() == 1 && !fuzzer.is_absolute() {
        fuzzer
    } else {
        resolve(fuzzer)
    };
    let cases = config.testing.cases.unwrap_or(25);
    if cases == 0 {
        return Err("testing.cases must be greater than zero".to_owned());
    }
    let policy = config.testing.policy.as_deref().unwrap_or("standard");
    let family = config
        .testing
        .scenario_family
        .as_deref()
        .unwrap_or("legacy");
    let mut cmd = ProcessCommand::new(&fuzzer);
    cmd.arg("fuzz")
        .arg("--baseline-runner")
        .arg(resolve(baseline))
        .arg("--candidate-runner")
        .arg(resolve(candidate))
        .arg("--policy")
        .arg(policy)
        .arg("--scenario-family")
        .arg(family)
        .arg("--seed")
        .arg(config.testing.seed.unwrap_or(0x5eed_cafe_u64).to_string())
        .arg("--cases")
        .arg(cases.to_string())
        .arg("--format")
        .arg("human");
    for arg in comparison.baseline_args {
        cmd.arg(format!("--baseline-arg={arg}"));
    }
    for arg in comparison.candidate_args {
        cmd.arg(format!("--candidate-arg={arg}"));
    }
    let output = cmd.output().map_err(|error|
        format!("could not launch fuzzer {}: {error}; build shieldspec-fuzz or set comparison.fuzzer_binary", fuzzer.display())
    )?;
    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    let code = output.status.code().unwrap_or(1);
    match code {
        0 => Ok(ExitCode::SUCCESS),
        2 => Ok(ExitCode::from(2)),
        _ => Ok(ExitCode::from(1)),
    }
}

fn execute_fixture(command: FixtureCommand) -> Result<ExitCode, String> {
    match command {
        FixtureCommand::List => {
            for name in fixture_names() {
                println!("{name}");
            }
        }
        FixtureCommand::Write { name, path } => {
            write_fixture(&name, &path).map_err(|error| error.to_string())?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn read_snapshot(path: &Path) -> Result<ProposalSnapshot, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let snapshot: ProposalSnapshot = serde_json::from_slice(&bytes)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
    validate_snapshot(&snapshot)
        .map_err(|error| format!("invalid snapshot {}: {error}", path.display()))?;
    Ok(snapshot)
}

fn print_report(
    passes: bool,
    findings: &[Finding],
    format: FormatName,
    source: &Path,
) -> Result<(), String> {
    let output = render(passes, findings, format.into(), source)?;
    print!("{output}");
    Ok(())
}

fn privacy_exit(passes: bool) -> ExitCode {
    if passes {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
