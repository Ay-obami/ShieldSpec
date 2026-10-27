use std::{
    io::{self, Read},
    process::ExitCode,
};

use clap::{Parser, ValueEnum};
use shieldspec_fuzz::RunnerResponse;
use shieldspec_zcash_runner::{
    run_scenario, run_scenario_with_orchard_only_spend_policy, Scenario,
};

#[derive(Debug, Parser)]
struct Args {
    /// Observe a genuine SDK spend-source policy verdict in addition to the snapshot.
    #[arg(long, value_enum)]
    wallet_spend_policy: Option<WalletSpendPolicy>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum WalletSpendPolicy {
    OrchardOnly,
}

fn main() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => return emit_error(format!("invalid runner arguments: {error}")),
    };

    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        return emit_error(format!("could not read scenario from stdin: {error}"));
    }

    let scenario: Scenario = match serde_json::from_str(&input) {
        Ok(scenario) => scenario,
        Err(error) => return emit_error(format!("invalid scenario JSON: {error}")),
    };

    let result = match args.wallet_spend_policy {
        Some(WalletSpendPolicy::OrchardOnly) => run_scenario_with_orchard_only_spend_policy(
            &scenario,
        )
        .map(|(snapshot, wallet_policy)| RunnerResponse::Observed {
            snapshot,
            wallet_policy,
        }),
        None => run_scenario(&scenario).map(|snapshot| RunnerResponse::Ok { snapshot }),
    };

    match result {
        Ok(response) => {
            if serde_json::to_writer(io::stdout(), &response).is_err() {
                return ExitCode::FAILURE;
            }
            println!();
            ExitCode::SUCCESS
        }
        Err(error) => emit_error(error.to_string()),
    }
}

fn emit_error(message: String) -> ExitCode {
    let response = RunnerResponse::Error { message };
    if serde_json::to_writer(io::stdout(), &response).is_ok() {
        println!();
    }
    ExitCode::FAILURE
}
