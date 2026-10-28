//! Copy this process boundary and replace ExampleWallet with a real wallet SDK.
//! The demo returns a synthetic empty snapshot, never evidence of a wallet check.
use serde_json::{json, Value};
use shieldspec_fuzz::{RunnerResponse, RUNNER_PROTOCOL_VERSION};
use std::{
    io::{self, Read},
    process::ExitCode,
};

trait WalletAdapter {
    fn execute(&self, scenario: &Value) -> Result<RunnerResponse, String>;
}

struct ExampleWallet {
    demo: bool,
}

impl WalletAdapter for ExampleWallet {
    fn execute(&self, scenario: &Value) -> Result<RunnerResponse, String> {
        let kind = scenario
            .get("kind")
            .and_then(Value::as_str)
            .ok_or("scenario.kind must be a string")?;
        let accepted = [
            "orchard_payment",
            "tex_payment",
            "unified_payment",
            "transparent_funded_payment",
            "nu63_orchard_payment",
            "zallet_privacy_probe",
        ];
        if !accepted.contains(&kind) || !scenario.get("params").is_some_and(Value::is_object) {
            return Err("unsupported scenario kind or missing params object".into());
        }
        if !self.demo {
            return Err("wallet not configured: replace ExampleWallet with your SDK adapter; --demo emits synthetic data only".into());
        }
        let snapshot = serde_json::from_value(json!({
            "schema_version": 1,
            "network": "test",
            "steps": [{
                "index": 0,
                "inputs": [{"pool": "orchard", "value_zat": 110000}],
                "outputs": [{"pool": "orchard", "role": "recipient", "value_zat": 100000}],
                "fee_zat": 10000
            }]
        }))
        .map_err(|error| format!("invalid demonstration snapshot: {error}"))?;
        Ok(RunnerResponse::Ok { snapshot })
    }
}

fn emit(response: RunnerResponse) -> ExitCode {
    let success = !matches!(response, RunnerResponse::Error { .. });
    let mut wire = serde_json::to_value(response).expect("runner response serializes");
    wire["protocol_version"] = json!(RUNNER_PROTOCOL_VERSION);
    if serde_json::to_writer(io::stdout(), &wire).is_err() {
        return ExitCode::FAILURE;
    }
    println!();
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() && args != ["--demo"] {
        return emit(RunnerResponse::Error {
            message: "only --demo is supported by the starter".into(),
        });
    }
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        return emit(RunnerResponse::Error {
            message: format!("stdin read failure: {error}"),
        });
    }
    let scenario: Value = match serde_json::from_str(&input) {
        Ok(value) => value,
        Err(error) => {
            return emit(RunnerResponse::Error {
                message: format!("invalid scenario JSON: {error}"),
            })
        }
    };
    match (ExampleWallet {
        demo: args == ["--demo"],
    })
    .execute(&scenario)
    {
        Ok(response) => emit(response),
        Err(message) => emit(RunnerResponse::Error { message }),
    }
}
