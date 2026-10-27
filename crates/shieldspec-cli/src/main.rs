mod reporter;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{Parser, Subcommand, ValueEnum};
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
