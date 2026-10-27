use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use shieldspec_core::{assess, Finding, Policy, ProposalSnapshot};
use shieldspec_fuzz::{
    run_differential, DifferentialProvenance, DifferentialReport, RunnerCommand, RunnerProvenance,
    RunnerResponse,
};
use shieldspec_zcash_runner::{
    run_scenario, shrink_scenario, Discovery, GeneratedCase, Scenario, ScenarioGenerator,
};

const PRIVACY_FAILURE_EXIT: u8 = 2;
const WALLET_POLICY_DIFFERENCE_RULE_ID: &str = "WALLET_POLICY_DECISION_DIFFERENCE";
const LIMIT_WARNING: &str =
    "No fuzzing result is a guarantee of privacy or anonymity, and a clean campaign only means no configured differential regression was found in the exercised scenarios.";

#[derive(Debug, Parser)]
#[command(
    name = "shieldspec-fuzz",
    about = "Generate real Zcash wallet proposal scenarios and fuzz privacy regressions"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run deterministic scenarios against independently compiled baseline and candidate runners.
    Fuzz {
        /// Deterministic legacy or activation-aware Ironwood scenario campaign.
        #[arg(long, value_enum, default_value_t = ScenarioFamily::Legacy)]
        scenario_family: ScenarioFamily,
        #[arg(long)]
        baseline_runner: PathBuf,
        #[arg(long)]
        candidate_runner: PathBuf,
        #[arg(long = "baseline-arg", allow_hyphen_values = true)]
        baseline_args: Vec<String>,
        #[arg(long = "candidate-arg", allow_hyphen_values = true)]
        candidate_args: Vec<String>,
        /// Exact SDK source version or Git SHA declared by the runner builder.
        #[arg(long)]
        baseline_sdk: Option<String>,
        /// Exact SDK source version or Git SHA declared by the runner builder.
        #[arg(long)]
        candidate_sdk: Option<String>,
        #[arg(long, default_value_t = 0x5eed_cafe_u64)]
        seed: u64,
        #[arg(long, default_value_t = 25)]
        cases: u64,
        #[arg(long, value_enum, default_value_t = PolicyName::Standard)]
        policy: PolicyName,
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
        /// Persist the first minimized differential discovery as JSON.
        #[arg(long)]
        save: Option<PathBuf>,
    },
    /// Replay a stored discovery, or reproduce a generated scenario by seed and ordinal.
    Replay {
        /// Source scenario family when replaying from a seed and ordinal.
        #[arg(long, value_enum, default_value_t = ScenarioFamily::Legacy)]
        scenario_family: ScenarioFamily,
        /// Discovery JSON to replay. Mutually exclusive with --seed/--ordinal.
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long)]
        ordinal: Option<u64>,
        #[arg(long)]
        baseline_runner: Option<PathBuf>,
        #[arg(long)]
        candidate_runner: Option<PathBuf>,
        #[arg(long = "baseline-arg", allow_hyphen_values = true)]
        baseline_args: Vec<String>,
        #[arg(long = "candidate-arg", allow_hyphen_values = true)]
        candidate_args: Vec<String>,
        /// Reproduce the built-in judge-demo candidate fault.
        #[arg(long, value_enum)]
        demo_regression: Option<DemoRegression>,
        #[arg(long, value_enum)]
        policy: Option<PolicyName>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Run the 30-second judge demo using the real proposal engine and a deterministic fault.
    Demo {
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
    /// Internal process-protocol runner used by the differential engine.
    #[command(name = "runner", hide = true)]
    Runner {
        #[arg(long, value_enum)]
        demo_regression: Option<DemoRegression>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
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

    const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Strict => "strict",
        }
    }

    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "standard" => Ok(Self::Standard),
            "strict" => Ok(Self::Strict),
            other => Err(format!("unsupported recorded policy: {other}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ScenarioFamily {
    Legacy,
    Ironwood,
}

impl ScenarioFamily {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Ironwood => "ironwood",
        }
    }

    const fn reproduction_arg(self) -> &'static str {
        match self {
            Self::Legacy => "",
            Self::Ironwood => " --scenario-family ironwood",
        }
    }

    fn generated_case(self, seed: u64, ordinal: u64) -> GeneratedCase {
        match self {
            Self::Legacy => ScenarioGenerator::case_at(seed, ordinal),
            Self::Ironwood => GeneratedCase {
                seed,
                ordinal,
                scenario: ScenarioGenerator::nu63_scenario_at(seed, ordinal),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DemoRegression {
    TransparentChange,
}

impl DemoRegression {
    const fn as_str(self) -> &'static str {
        match self {
            Self::TransparentChange => "transparent-change",
        }
    }
}

#[derive(Debug, Serialize)]
struct CampaignOutput {
    status: &'static str,
    scenario_family: &'static str,
    seed: u64,
    cases_executed: u64,
    baseline_wallet_policy_observations: u64,
    candidate_wallet_policy_observations: u64,
    comparable_wallet_policy_cases: u64,
    ordinal: Option<u64>,
    scenario: Option<Scenario>,
    minimized_scenario: Option<Scenario>,
    report: Option<DifferentialReport>,
    reproduction: Option<String>,
    warning: &'static str,
}

#[derive(Debug, Serialize)]
struct ReplayOutput {
    status: &'static str,
    scenario: Scenario,
    findings: Vec<Finding>,
    differential_report: Option<DifferentialReport>,
    recorded_rule_id: Option<String>,
    recorded_rule_reproduced: Option<bool>,
    reproduction: String,
    warning: &'static str,
}

#[derive(Debug, Serialize)]
struct DifferentialReplayOutput {
    status: &'static str,
    scenario: Scenario,
    report: DifferentialReport,
    recorded_rule_id: Option<String>,
    recorded_rule_reproduced: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provenance_verified: Option<bool>,
    reproduction: String,
    warning: &'static str,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Fuzz {
            scenario_family,
            baseline_runner,
            candidate_runner,
            baseline_args,
            candidate_args,
            baseline_sdk,
            candidate_sdk,
            seed,
            cases,
            policy,
            format,
            save,
        } => {
            if cases == 0 {
                Err("cases must be greater than zero".to_owned())
            } else {
                let baseline = runner_command(baseline_runner, baseline_args);
                let candidate = runner_command(candidate_runner, candidate_args);
                run_campaign(
                    &baseline,
                    &candidate,
                    seed,
                    cases,
                    scenario_family,
                    policy,
                    format,
                    save.as_deref(),
                    baseline_sdk,
                    candidate_sdk,
                )
            }
        }
        Command::Replay {
            scenario_family,
            input,
            seed,
            ordinal,
            baseline_runner,
            candidate_runner,
            baseline_args,
            candidate_args,
            demo_regression,
            policy,
            format,
        } => replay(
            input.as_deref(),
            seed,
            ordinal,
            scenario_family,
            baseline_runner,
            candidate_runner,
            baseline_args,
            candidate_args,
            demo_regression,
            policy,
            format,
        ),
        Command::Demo { format } => demo(format),
        Command::Runner { demo_regression } => return internal_runner(demo_regression),
    };

    match result {
        Ok(code) => code,
        Err(message) => {
            eprintln!("shieldspec-fuzz: {message}");
            ExitCode::from(1)
        }
    }
}

fn runner_command(program: PathBuf, args: Vec<String>) -> RunnerCommand {
    args.into_iter()
        .fold(RunnerCommand::new(program), |command, arg| command.arg(arg))
}

#[allow(clippy::too_many_arguments)]
fn run_campaign(
    baseline: &RunnerCommand,
    candidate: &RunnerCommand,
    seed: u64,
    cases: u64,
    scenario_family: ScenarioFamily,
    policy_name: PolicyName,
    format: OutputFormat,
    save: Option<&Path>,
    baseline_sdk: Option<String>,
    candidate_sdk: Option<String>,
) -> Result<ExitCode, String> {
    let policy = policy_name.policy();
    let provenance = if save.is_some() {
        Some(DifferentialProvenance {
            baseline: RunnerProvenance::capture(baseline, baseline_sdk)?,
            candidate: RunnerProvenance::capture(candidate, candidate_sdk)?,
            policy: policy_name.as_str().to_owned(),
        })
    } else {
        None
    };
    let mut legacy_generator = ScenarioGenerator::new(seed);
    let mut baseline_wallet_policy_observations = 0;
    let mut candidate_wallet_policy_observations = 0;
    let mut comparable_wallet_policy_cases = 0;

    for case_number in 1..=cases {
        let generated = match scenario_family {
            ScenarioFamily::Legacy => legacy_generator.next_case(),
            ScenarioFamily::Ironwood => scenario_family.generated_case(seed, case_number - 1),
        };
        let scenario_json =
            serde_json::to_value(&generated.scenario).map_err(|error| error.to_string())?;
        let report =
            run_differential(baseline, candidate, &scenario_json, &policy).map_err(|error| {
                format!(
                    "case {} (ordinal {}): {error}",
                    case_number, generated.ordinal
                )
            })?;
        if report.baseline_wallet_policy.is_some() {
            baseline_wallet_policy_observations += 1;
        }
        if report.candidate_wallet_policy.is_some() {
            candidate_wallet_policy_observations += 1;
        }
        if report.baseline_wallet_policy.is_some() && report.candidate_wallet_policy.is_some() {
            comparable_wallet_policy_cases += 1;
        }

        if report.has_difference() {
            let original = generated.scenario.clone();
            // Preserve the original wallet verdict change when present; a different
            // snapshot finding must never substitute for a lost wallet observation.
            let minimized = shrink_scenario(&original, |scenario| {
                let Ok(value) = serde_json::to_value(scenario) else {
                    return false;
                };
                run_differential(baseline, candidate, &value, &policy)
                    .map(|next| next.preserves_discovery(&report))
                    .unwrap_or(false)
            });

            let minimized_value =
                serde_json::to_value(&minimized).map_err(|error| error.to_string())?;
            let minimized_report = run_differential(baseline, candidate, &minimized_value, &policy)
                .map_err(|error| format!("minimized case could not replay: {error}"))?;
            if !minimized_report.preserves_discovery(&report) {
                return Err(format!(
                    "minimized discovery no longer reproduces the original finding at seed {seed}, ordinal {} (runner behavior may be nondeterministic)",
                    generated.ordinal
                ));
            }
            let original_rerun = run_differential(baseline, candidate, &scenario_json, &policy)
                .map_err(|error| format!("original scenario could not replay: {error}"))?;
            if !original_rerun.preserves_discovery(&report) {
                return Err(format!(
                    "original scenario no longer reproduces its initial finding at seed {seed}, ordinal {} (runner behavior may be nondeterministic)",
                    generated.ordinal
                ));
            }

            let rule_id = if report.wallet_policy_difference.is_some() {
                WALLET_POLICY_DIFFERENCE_RULE_ID.to_owned()
            } else {
                report
                    .regression
                    .findings
                    .first()
                    .map(|finding| finding.rule_id.clone())
                    .unwrap_or_else(|| "UNKNOWN_REGRESSION".to_owned())
            };

            if let Some(path) = save {
                let discovery = Discovery {
                    seed,
                    ordinal: generated.ordinal,
                    scenario_family: (scenario_family == ScenarioFamily::Ironwood)
                        .then(|| scenario_family.as_str().to_owned()),
                    rule_id,
                    original: original.clone(),
                    minimized: minimized.clone(),
                    provenance: provenance.clone(),
                    wallet_policy_difference: minimized_report.wallet_policy_difference.clone(),
                };
                write_pretty_json(path, &discovery)?;
            }

            let reproduction = differential_reproduction(
                seed,
                generated.ordinal,
                scenario_family,
                policy_name,
                baseline,
                candidate,
            );
            let status = if minimized_report.wallet_policy_difference.is_some() {
                "wallet_policy_difference"
            } else {
                "privacy_regression"
            };
            let output = CampaignOutput {
                status,
                scenario_family: scenario_family.as_str(),
                seed,
                cases_executed: case_number,
                baseline_wallet_policy_observations,
                candidate_wallet_policy_observations,
                comparable_wallet_policy_cases,
                ordinal: Some(generated.ordinal),
                scenario: Some(original),
                minimized_scenario: Some(minimized),
                report: Some(minimized_report),
                reproduction: Some(reproduction),
                warning: LIMIT_WARNING,
            };
            render_campaign(&output, format)?;
            return Ok(ExitCode::from(PRIVACY_FAILURE_EXIT));
        }
    }

    let output = CampaignOutput {
        status: "no_regression_found",
        scenario_family: scenario_family.as_str(),
        seed,
        cases_executed: cases,
        baseline_wallet_policy_observations,
        candidate_wallet_policy_observations,
        comparable_wallet_policy_cases,
        ordinal: None,
        scenario: None,
        minimized_scenario: None,
        report: None,
        reproduction: None,
        warning: LIMIT_WARNING,
    };
    render_campaign(&output, format)?;
    Ok(ExitCode::SUCCESS)
}

#[allow(clippy::too_many_arguments)]
fn replay(
    input: Option<&Path>,
    seed: Option<u64>,
    ordinal: Option<u64>,
    scenario_family: ScenarioFamily,
    baseline_runner: Option<PathBuf>,
    candidate_runner: Option<PathBuf>,
    baseline_args: Vec<String>,
    candidate_args: Vec<String>,
    demo_regression: Option<DemoRegression>,
    policy_name: Option<PolicyName>,
    format: OutputFormat,
) -> Result<ExitCode, String> {
    let (
        scenario,
        recorded_rule_id,
        replay_seed,
        replay_ordinal,
        replay_scenario_family,
        single_reproduction,
        saved_provenance,
        saved_wallet_policy_difference,
    ) = match (input, seed, ordinal) {
        (Some(path), None, None) => {
            let bytes = fs::read(path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            let discovery: Discovery = serde_json::from_slice(&bytes)
                .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
            let inferred_family = match &discovery.original {
                Scenario::Nu63OrchardPayment(_) => ScenarioFamily::Ironwood,
                Scenario::OrchardPayment(_) | Scenario::TexPayment(_) => ScenarioFamily::Legacy,
            };
            let recorded_family = match discovery.scenario_family.as_deref() {
                None => inferred_family,
                Some("legacy") => ScenarioFamily::Legacy,
                Some("ironwood") => ScenarioFamily::Ironwood,
                Some(other) => {
                    return Err(format!("unsupported recorded scenario family: {other}"))
                }
            };
            if recorded_family != inferred_family {
                return Err(
                    "recorded scenario family conflicts with the original scenario kind".to_owned(),
                );
            }
            (
                    discovery.minimized,
                    Some(discovery.rule_id),
                    discovery.seed,
                    discovery.ordinal,
                    recorded_family,
                    format!(
                        "cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- replay --input {}",
                        path.display()
                    ),
                    discovery.provenance,
                    discovery.wallet_policy_difference,
                )
        }
        (None, Some(seed), Some(ordinal)) => {
            let generated = scenario_family.generated_case(seed, ordinal);
            (
                    generated.scenario,
                    None,
                    seed,
                    ordinal,
                    scenario_family,
                    format!(
                        "cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- replay --seed {seed} --ordinal {ordinal}{}",
                        scenario_family.reproduction_arg(),
                    ),
                    None,
                    None,
                )
        }
        _ => {
            return Err(
                "provide either --input <discovery.json> or both --seed <n> and --ordinal <n>"
                    .to_owned(),
            )
        }
    };

    let policy_name = match (policy_name, saved_provenance.as_ref()) {
        (Some(policy), Some(saved)) if policy.as_str() != saved.policy => {
            return Err(format!(
                "replay policy {} differs from recorded policy {}",
                policy.as_str(),
                saved.policy
            ));
        }
        (Some(policy), _) => policy,
        (None, Some(saved)) => PolicyName::from_str(&saved.policy)?,
        (None, None) => PolicyName::Standard,
    };

    let differential_pair = if let Some(regression) = demo_regression {
        if baseline_runner.is_some()
            || candidate_runner.is_some()
            || !baseline_args.is_empty()
            || !candidate_args.is_empty()
        {
            return Err(
                "--demo-regression cannot be combined with explicit baseline/candidate runners"
                    .to_owned(),
            );
        }
        let executable = std::env::current_exe()
            .map_err(|error| format!("could not locate current binary: {error}"))?;
        Some((
            RunnerCommand::new(&executable).arg("runner"),
            RunnerCommand::new(&executable)
                .arg("runner")
                .arg("--demo-regression")
                .arg(regression.as_str()),
        ))
    } else {
        match (baseline_runner, candidate_runner) {
            (Some(baseline), Some(candidate)) => Some((
                runner_command(baseline, baseline_args),
                runner_command(candidate, candidate_args),
            )),
            (None, None) if baseline_args.is_empty() && candidate_args.is_empty() => None,
            (None, None) => {
                return Err(
                    "--baseline-arg/--candidate-arg require their corresponding runner".to_owned(),
                )
            }
            _ => {
                return Err(
                    "provide both --baseline-runner and --candidate-runner for differential replay"
                        .to_owned(),
                )
            }
        }
    };

    // Prefer the saved identities when a discovery has them, but permit an
    // explicitly supplied executable in another directory if its bytes match.
    let differential_pair = differential_pair.or_else(|| {
        saved_provenance
            .as_ref()
            .map(|saved| (saved.baseline.command(), saved.candidate.command()))
    });

    if let (Some(saved), Some((baseline, candidate))) =
        (saved_provenance.as_ref(), differential_pair.as_ref())
    {
        saved.baseline.verify(baseline)?;
        saved.candidate.verify(candidate)?;
    }

    if let Some((baseline, candidate)) = differential_pair {
        let scenario_json = serde_json::to_value(&scenario).map_err(|error| error.to_string())?;
        let report = run_differential(&baseline, &candidate, &scenario_json, &policy_name.policy())
            .map_err(|error| error.to_string())?;
        let recorded_rule_reproduced = recorded_rule_id.as_ref().map(|rule_id| {
            if rule_id == WALLET_POLICY_DIFFERENCE_RULE_ID {
                saved_wallet_policy_difference
                    .as_ref()
                    .is_some_and(|expected| {
                        report.wallet_policy_difference.as_ref() == Some(expected)
                    })
            } else {
                report
                    .regression
                    .findings
                    .iter()
                    .any(|finding| &finding.rule_id == rule_id)
            }
        });
        let reproduction = if input.is_some() {
            // An --input discovery executes its minimized scenario. A seed-only
            // command would regenerate the original input and misstate the
            // replayed evidence when shrinking changed the scenario.
            let mut command = format!("{single_reproduction} --policy {}", policy_name.as_str());
            if saved_provenance.is_none() {
                if let Some(regression) = demo_regression_from(&candidate) {
                    command.push_str(&format!(" --demo-regression {}", regression.as_str()));
                } else {
                    command.push_str(&format!(
                        " --baseline-runner {} --candidate-runner {}",
                        shell_arg(&baseline.program.to_string_lossy()),
                        shell_arg(&candidate.program.to_string_lossy())
                    ));
                    for arg in &baseline.args {
                        command.push_str(" --baseline-arg ");
                        command.push_str(&shell_arg(arg));
                    }
                    for arg in &candidate.args {
                        command.push_str(" --candidate-arg ");
                        command.push_str(&shell_arg(arg));
                    }
                }
            }
            command
        } else {
            differential_reproduction(
                replay_seed,
                replay_ordinal,
                replay_scenario_family,
                policy_name,
                &baseline,
                &candidate,
            )
        };
        let output = DifferentialReplayOutput {
            status: if recorded_rule_reproduced == Some(false) {
                if report.wallet_policy_difference.is_some() {
                    "other_wallet_policy_difference"
                } else if !report.regression.passes {
                    "other_privacy_regression"
                } else {
                    "recorded_finding_not_reproduced"
                }
            } else if recorded_rule_id.as_deref() == Some(WALLET_POLICY_DIFFERENCE_RULE_ID) {
                "wallet_policy_difference_reproduced"
            } else if recorded_rule_id.is_some() {
                "privacy_regression_reproduced"
            } else if report.wallet_policy_difference.is_some() {
                "wallet_policy_difference_reproduced"
            } else if !report.regression.passes {
                "privacy_regression_reproduced"
            } else {
                "no_regression"
            },
            scenario,
            report,
            recorded_rule_id,
            recorded_rule_reproduced,
            provenance_verified: saved_provenance.as_ref().map(|_| true),
            reproduction,
            warning: LIMIT_WARNING,
        };
        let failed = output.report.has_difference();
        render_differential_replay(&output, format)?;
        return Ok(if failed {
            ExitCode::from(PRIVACY_FAILURE_EXIT)
        } else {
            ExitCode::SUCCESS
        });
    }

    let snapshot = run_scenario(&scenario).map_err(|error| error.to_string())?;
    let assessment = assess(&snapshot, &policy_name.policy());
    let recorded_rule_reproduced = recorded_rule_id.as_ref().map(|rule_id| {
        assessment
            .findings
            .iter()
            .any(|finding| &finding.rule_id == rule_id)
    });

    let output = ReplayOutput {
        status: if assessment.passes {
            "policy_pass"
        } else {
            "privacy_finding_reproduced"
        },
        scenario,
        findings: assessment.findings,
        differential_report: None,
        recorded_rule_id,
        recorded_rule_reproduced,
        reproduction: format!("{single_reproduction} --policy {}", policy_name.as_str()),
        warning: LIMIT_WARNING,
    };
    let failed = !output.findings.is_empty();
    render_replay(&output, format)?;

    Ok(if failed {
        ExitCode::from(PRIVACY_FAILURE_EXIT)
    } else {
        ExitCode::SUCCESS
    })
}

fn differential_reproduction(
    seed: u64,
    ordinal: u64,
    scenario_family: ScenarioFamily,
    policy_name: PolicyName,
    baseline: &RunnerCommand,
    candidate: &RunnerCommand,
) -> String {
    if let Some(regression) = demo_regression_from(candidate) {
        return format!(
            "cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- replay --seed {seed} --ordinal {ordinal}{} --policy {} --demo-regression {}",
            scenario_family.reproduction_arg(),
            policy_name.as_str(),
            regression.as_str()
        );
    }

    let mut command = format!(
        "cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- replay --seed {seed} --ordinal {ordinal}{} --policy {} --baseline-runner {} --candidate-runner {}",
        scenario_family.reproduction_arg(),
        policy_name.as_str(),
        shell_arg(&baseline.program.to_string_lossy()),
        shell_arg(&candidate.program.to_string_lossy())
    );
    for arg in &baseline.args {
        command.push_str(" --baseline-arg ");
        command.push_str(&shell_arg(arg));
    }
    for arg in &candidate.args {
        command.push_str(" --candidate-arg ");
        command.push_str(&shell_arg(arg));
    }
    command
}

fn demo_regression_from(command: &RunnerCommand) -> Option<DemoRegression> {
    command.args.windows(2).find_map(|pair| {
        (pair[0] == "--demo-regression" && pair[1] == "transparent-change")
            .then_some(DemoRegression::TransparentChange)
    })
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-_./:".contains(character))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn demo(format: OutputFormat) -> Result<ExitCode, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate current binary: {error}"))?;
    let baseline = RunnerCommand::new(&executable).arg("runner");
    let candidate = RunnerCommand::new(&executable)
        .arg("runner")
        .arg("--demo-regression")
        .arg("transparent-change");

    run_campaign(
        &baseline,
        &candidate,
        148,
        1,
        ScenarioFamily::Legacy,
        PolicyName::Standard,
        format,
        None,
        None,
        None,
    )
}

fn internal_runner(demo_regression: Option<DemoRegression>) -> ExitCode {
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        return emit_runner_error(format!("could not read scenario from stdin: {error}"));
    }

    let scenario: Scenario = match serde_json::from_str(&input) {
        Ok(scenario) => scenario,
        Err(error) => return emit_runner_error(format!("invalid scenario JSON: {error}")),
    };

    match run_scenario(&scenario) {
        Ok(mut snapshot) => {
            if let Some(regression) = demo_regression {
                if let Err(error) = apply_demo_regression(&mut snapshot, regression) {
                    return emit_runner_error(error);
                }
            }
            emit_runner_response(RunnerResponse::Ok { snapshot }, ExitCode::SUCCESS)
        }
        Err(error) => emit_runner_error(error.to_string()),
    }
}

fn apply_demo_regression(
    snapshot: &mut ProposalSnapshot,
    regression: DemoRegression,
) -> Result<(), String> {
    match regression {
        DemoRegression::TransparentChange => {
            let output = snapshot
                .steps
                .iter_mut()
                .flat_map(|step| step.outputs.iter_mut())
                .find(|output| {
                    output.role == shieldspec_core::OutputRole::Change
                        && output.pool != shieldspec_core::Pool::Transparent
                })
                .ok_or_else(|| {
                    "demo fault injection requires a shielded change output in the real proposal"
                        .to_owned()
                })?;
            output.pool = shieldspec_core::Pool::Transparent;
            Ok(())
        }
    }
}

fn emit_runner_error(message: String) -> ExitCode {
    emit_runner_response(RunnerResponse::Error { message }, ExitCode::from(1))
}

fn emit_runner_response(response: RunnerResponse, code: ExitCode) -> ExitCode {
    if serde_json::to_writer(io::stdout(), &response).is_err() {
        return ExitCode::from(1);
    }
    println!();
    code
}

fn write_pretty_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    json.push('\n');
    fs::write(path, json).map_err(|error| format!("could not write {}: {error}", path.display()))
}

fn render_campaign(output: &CampaignOutput, format: OutputFormat) -> Result<(), String> {
    match format {
        OutputFormat::Json => print_json(output),
        OutputFormat::Human => {
            println!("ShieldSpec Privacy Differential Fuzzer");
            println!("status: {}", output.status);
            println!("seed: {}", output.seed);
            println!("scenario family: {}", output.scenario_family);
            println!("cases executed: {}", output.cases_executed);
            println!(
                "wallet policy evidence: baseline {}, candidate {}, comparable {}",
                output.baseline_wallet_policy_observations,
                output.candidate_wallet_policy_observations,
                output.comparable_wallet_policy_cases,
            );
            if let Some(ordinal) = output.ordinal {
                println!("failing ordinal: {ordinal}");
            }
            if let Some(scenario) = &output.minimized_scenario {
                println!(
                    "minimal scenario: {}",
                    serde_json::to_string(scenario).map_err(|error| error.to_string())?
                );
            }
            if let Some(report) = &output.report {
                print_findings("baseline findings", &report.baseline.findings);
                print_findings("candidate findings", &report.candidate.findings);
                print_findings(
                    "new/worsened privacy regressions",
                    &report.regression.findings,
                );
                print_wallet_policy_difference(report);
            }
            if let Some(reproduction) = &output.reproduction {
                println!("reproduce: {reproduction}");
            }
            println!("note: {}", output.warning);
            Ok(())
        }
    }
}

fn render_differential_replay(
    output: &DifferentialReplayOutput,
    format: OutputFormat,
) -> Result<(), String> {
    match format {
        OutputFormat::Json => print_json(output),
        OutputFormat::Human => {
            println!("ShieldSpec differential replay");
            println!("status: {}", output.status);
            println!(
                "scenario: {}",
                serde_json::to_string(&output.scenario).map_err(|error| error.to_string())?
            );
            print_findings("baseline findings", &output.report.baseline.findings);
            print_findings("candidate findings", &output.report.candidate.findings);
            print_findings(
                "new/worsened privacy regressions",
                &output.report.regression.findings,
            );
            print_wallet_policy_difference(&output.report);
            if let Some(rule_id) = &output.recorded_rule_id {
                println!("recorded rule: {rule_id}");
                println!(
                    "recorded rule reproduced: {}",
                    output.recorded_rule_reproduced.unwrap_or(false)
                );
            }
            println!("reproduce: {}", output.reproduction);
            println!("note: {}", output.warning);
            Ok(())
        }
    }
}

fn render_replay(output: &ReplayOutput, format: OutputFormat) -> Result<(), String> {
    match format {
        OutputFormat::Json => print_json(output),
        OutputFormat::Human => {
            println!("ShieldSpec replay");
            println!("status: {}", output.status);
            println!(
                "scenario: {}",
                serde_json::to_string(&output.scenario).map_err(|error| error.to_string())?
            );
            print_findings("findings", &output.findings);
            if let Some(rule_id) = &output.recorded_rule_id {
                println!("recorded rule: {rule_id}");
                println!(
                    "recorded rule reproduced: {}",
                    output.recorded_rule_reproduced.unwrap_or(false)
                );
            }
            println!("reproduce: {}", output.reproduction);
            println!("note: {}", output.warning);
            Ok(())
        }
    }
}

fn print_wallet_policy_difference(report: &DifferentialReport) {
    if let Some(difference) = &report.wallet_policy_difference {
        println!(
            "observed wallet policy {}: baseline {:?}, candidate {:?}",
            difference.policy, difference.baseline, difference.candidate
        );
    }
}

fn print_findings(label: &str, findings: &[Finding]) {
    println!("{label}: {}", findings.len());
    for finding in findings {
        println!(
            "  - {} [{:?}] step {}: {}",
            finding.rule_id, finding.severity, finding.step_index, finding.message
        );
    }
}

fn print_json(value: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer_pretty(io::stdout(), value).map_err(|error| error.to_string())?;
    println!();
    Ok(())
}
