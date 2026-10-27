use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use shieldspec_core::{assess, Finding, Policy, ProposalSnapshot};
use shieldspec_fuzz::{run_differential, DifferentialReport, RunnerCommand, RunnerResponse};
use shieldspec_zcash_runner::{
    run_scenario, shrink_scenario, Discovery, Scenario, ScenarioGenerator,
};

const PRIVACY_FAILURE_EXIT: u8 = 2;
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
        #[arg(long)]
        baseline_runner: PathBuf,
        #[arg(long)]
        candidate_runner: PathBuf,
        #[arg(long = "baseline-arg", allow_hyphen_values = true)]
        baseline_args: Vec<String>,
        #[arg(long = "candidate-arg", allow_hyphen_values = true)]
        candidate_args: Vec<String>,
        #[arg(long, default_value_t = 0x5eed_cafe_u64)]
        seed: u64,
        #[arg(long, default_value_t = 25)]
        cases: u64,
        /// Opt into real transparent-UTXO funding cases (legacy seeds remain unchanged by default).
        #[arg(long)]
        transparent_funding: bool,
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
        /// Discovery JSON to replay. Mutually exclusive with --seed/--ordinal.
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long)]
        ordinal: Option<u64>,
        /// Replay an extended-generator seed containing transparent funding.
        #[arg(long)]
        transparent_funding: bool,
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
        #[arg(long, value_enum, default_value_t = PolicyName::Standard)]
        policy: PolicyName,
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

    const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Strict => "strict",
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
    seed: u64,
    cases_executed: u64,
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
    reproduction: String,
    warning: &'static str,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Fuzz {
            baseline_runner,
            candidate_runner,
            baseline_args,
            candidate_args,
            seed,
            cases,
            transparent_funding,
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
                    transparent_funding,
                    policy,
                    format,
                    save.as_deref(),
                )
            }
        }
        Command::Replay {
            input,
            seed,
            ordinal,
            transparent_funding,
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
            transparent_funding,
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

fn run_campaign(
    baseline: &RunnerCommand,
    candidate: &RunnerCommand,
    seed: u64,
    cases: u64,
    transparent_funding: bool,
    policy_name: PolicyName,
    format: OutputFormat,
    save: Option<&Path>,
) -> Result<ExitCode, String> {
    let policy = policy_name.policy();
    let mut generator = if transparent_funding {
        ScenarioGenerator::new_with_transparent_funding(seed)
    } else {
        ScenarioGenerator::new(seed)
    };

    for case_number in 1..=cases {
        let generated = generator.next_case();
        let scenario_json =
            serde_json::to_value(&generated.scenario).map_err(|error| error.to_string())?;
        let report =
            run_differential(baseline, candidate, &scenario_json, &policy).map_err(|error| {
                format!(
                    "case {} (ordinal {}): {error}",
                    case_number, generated.ordinal
                )
            })?;

        if !report.regression.passes {
            let original = generated.scenario.clone();
            let minimized = shrink_scenario(&original, |scenario| {
                let Ok(value) = serde_json::to_value(scenario) else {
                    return false;
                };
                run_differential(baseline, candidate, &value, &policy)
                    .map(|report| !report.regression.passes)
                    .unwrap_or(false)
            });

            let minimized_value =
                serde_json::to_value(&minimized).map_err(|error| error.to_string())?;
            let minimized_report = run_differential(baseline, candidate, &minimized_value, &policy)
                .map_err(|error| format!("minimized case could not replay: {error}"))?;

            let rule_id = minimized_report
                .regression
                .findings
                .first()
                .map(|finding| finding.rule_id.clone())
                .unwrap_or_else(|| "UNKNOWN_REGRESSION".to_owned());

            if let Some(path) = save {
                let discovery = Discovery {
                    seed,
                    ordinal: generated.ordinal,
                    rule_id,
                    original: original.clone(),
                    minimized: minimized.clone(),
                };
                write_pretty_json(path, &discovery)?;
            }

            let reproduction = differential_reproduction(
                seed,
                generated.ordinal,
                transparent_funding,
                policy_name,
                baseline,
                candidate,
            );
            let output = CampaignOutput {
                status: "privacy_regression",
                seed,
                cases_executed: case_number,
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
        seed,
        cases_executed: cases,
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
    transparent_funding: bool,
    baseline_runner: Option<PathBuf>,
    candidate_runner: Option<PathBuf>,
    baseline_args: Vec<String>,
    candidate_args: Vec<String>,
    demo_regression: Option<DemoRegression>,
    policy_name: PolicyName,
    format: OutputFormat,
) -> Result<ExitCode, String> {
    let (scenario, recorded_rule_id, replay_seed, replay_ordinal, single_reproduction) = match (
        input, seed, ordinal,
    ) {
        (Some(path), None, None) => {
            let bytes = fs::read(path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            let discovery: Discovery = serde_json::from_slice(&bytes)
                .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
            (
                    discovery.minimized,
                    Some(discovery.rule_id),
                    discovery.seed,
                    discovery.ordinal,
                    format!(
                        "cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- replay --input {}",
                        path.display()
                    ),
                )
        }
        (None, Some(seed), Some(ordinal)) => {
            let generated = if transparent_funding {
                ScenarioGenerator::case_at_with_transparent_funding(seed, ordinal)
            } else {
                ScenarioGenerator::case_at(seed, ordinal)
            };
            (
                    generated.scenario,
                    None,
                    seed,
                    ordinal,
                    format!(
                        "cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- replay --seed {seed} --ordinal {ordinal}{}",
                        if transparent_funding { " --transparent-funding" } else { "" }
                    ),
                )
        }
        _ => {
            return Err(
                "provide either --input <discovery.json> or both --seed <n> and --ordinal <n>"
                    .to_owned(),
            )
        }
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

    if let Some((baseline, candidate)) = differential_pair {
        let scenario_json = serde_json::to_value(&scenario).map_err(|error| error.to_string())?;
        let report = run_differential(&baseline, &candidate, &scenario_json, &policy_name.policy())
            .map_err(|error| error.to_string())?;
        let recorded_rule_reproduced = recorded_rule_id.as_ref().map(|rule_id| {
            report
                .regression
                .findings
                .iter()
                .any(|finding| &finding.rule_id == rule_id)
        });
        let reproduction = differential_reproduction(
            replay_seed,
            replay_ordinal,
            transparent_funding,
            policy_name,
            &baseline,
            &candidate,
        );
        let output = DifferentialReplayOutput {
            status: if report.regression.passes {
                "no_regression"
            } else {
                "privacy_regression_reproduced"
            },
            scenario,
            report,
            recorded_rule_id,
            recorded_rule_reproduced,
            reproduction,
            warning: LIMIT_WARNING,
        };
        let failed = !output.report.regression.passes;
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
    transparent_funding: bool,
    policy_name: PolicyName,
    baseline: &RunnerCommand,
    candidate: &RunnerCommand,
) -> String {
    if let Some(regression) = demo_regression_from(candidate) {
        return format!(
            "cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- replay --seed {seed} --ordinal {ordinal}{} --policy {} --demo-regression {}",
            if transparent_funding { " --transparent-funding" } else { "" },
            policy_name.as_str(),
            regression.as_str()
        );
    }

    let mut command = format!(
        "cargo run --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz -- replay --seed {seed} --ordinal {ordinal}{} --policy {} --baseline-runner {} --candidate-runner {}",
        if transparent_funding { " --transparent-funding" } else { "" },
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
        false,
        PolicyName::Standard,
        format,
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
            println!("cases executed: {}", output.cases_executed);
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
