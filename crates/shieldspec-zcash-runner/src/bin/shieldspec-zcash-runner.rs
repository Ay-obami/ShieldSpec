use std::{
    io::{self, Read},
    process::ExitCode,
};

use shieldspec_fuzz::RunnerResponse;
use shieldspec_zcash_runner::{run_scenario, Scenario};

fn main() -> ExitCode {
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        return emit_error(format!("could not read scenario from stdin: {error}"));
    }

    let scenario: Scenario = match serde_json::from_str(&input) {
        Ok(scenario) => scenario,
        Err(error) => return emit_error(format!("invalid scenario JSON: {error}")),
    };

    match run_scenario(&scenario) {
        Ok(snapshot) => {
            let response = RunnerResponse::Ok { snapshot };
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
