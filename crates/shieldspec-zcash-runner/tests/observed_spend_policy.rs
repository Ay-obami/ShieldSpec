use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

use shieldspec_core::{OutputRole, Pool};
use shieldspec_fuzz::{RunnerResponse, WalletPolicyDecision, WalletPolicyVerdict};
use shieldspec_zcash_runner::{OrchardPaymentScenario, Scenario};

fn execute(args: &[&str], scenario: &Scenario) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_shieldspec-zcash-runner"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch real Zcash wallet runner");

    let input = serde_json::to_vec(scenario).expect("serialize scenario");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(&input)
        .expect("write scenario");

    child.wait_with_output().expect("runner output")
}

fn response(output: &Output) -> RunnerResponse {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "expected runner JSON ({error}); stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn orchard_only_policy_accepts_real_orchard_funding() {
    let scenario = Scenario::OrchardPayment(OrchardPaymentScenario::demo());
    let output = execute(&["--wallet-spend-policy", "orchard-only"], &scenario);
    assert!(
        output.status.success(),
        "runner stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let RunnerResponse::Observed {
        snapshot,
        wallet_policy,
    } = response(&output)
    else {
        panic!("opt-in runner must emit an observed wallet verdict");
    };

    assert_eq!(
        wallet_policy,
        WalletPolicyVerdict {
            policy: "zcash_spend_orchard_only_v1".to_owned(),
            decision: WalletPolicyDecision::Accepted,
        }
    );
    assert_eq!(snapshot.network, "local-test");
    assert_eq!(snapshot.steps.len(), 1);
    assert!(snapshot.steps[0]
        .inputs
        .iter()
        .any(|input| input.pool == Pool::Orchard));
    assert!(snapshot.steps[0].outputs.iter().any(|output| {
        output.pool == Pool::Orchard
            && output.role == OutputRole::Recipient
            && output.value_zat == 100_000
    }));
}

#[test]
fn orchard_only_policy_rejects_real_sapling_funding_with_diagnostic_snapshot() {
    use shieldspec_zcash_runner::ShieldedPoolChoice;

    let scenario = Scenario::OrchardPayment(OrchardPaymentScenario {
        funding_pool: ShieldedPoolChoice::Sapling,
        ..OrchardPaymentScenario::demo()
    });
    let output = execute(&["--wallet-spend-policy", "orchard-only"], &scenario);
    assert!(
        output.status.success(),
        "policy rejection is not an operational failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let RunnerResponse::Observed {
        snapshot,
        wallet_policy,
    } = response(&output)
    else {
        panic!("opt-in runner must emit an observed wallet verdict");
    };

    assert_eq!(
        wallet_policy,
        WalletPolicyVerdict {
            policy: "zcash_spend_orchard_only_v1".to_owned(),
            decision: WalletPolicyDecision::Rejected,
        }
    );
    assert_eq!(snapshot.network, "local-test");
    assert!(!snapshot.steps.is_empty());
    assert!(snapshot
        .steps
        .iter()
        .flat_map(|step| step.inputs.iter())
        .any(|input| input.pool == Pool::Sapling));
}

#[test]
fn default_runner_remains_snapshot_only_for_legacy_callers() {
    let scenario = Scenario::OrchardPayment(OrchardPaymentScenario::demo());
    let output = execute(&[], &scenario);
    assert!(output.status.success());

    let RunnerResponse::Ok { snapshot } = response(&output) else {
        panic!("legacy runner must retain status ok without a wallet verdict");
    };
    assert_eq!(snapshot.network, "local-test");
    assert_eq!(snapshot.steps.len(), 1);
}

#[test]
fn insufficient_unrestricted_funds_is_an_operational_error_not_policy_rejection() {
    let scenario = Scenario::OrchardPayment(OrchardPaymentScenario {
        payment_zat: 119_999,
        ..OrchardPaymentScenario::demo()
    });
    let output = execute(&["--wallet-spend-policy", "orchard-only"], &scenario);
    assert!(!output.status.success());

    let RunnerResponse::Error { message } = response(&output) else {
        panic!("failure to construct the diagnostic proposal must be an error");
    };
    assert!(
        message.contains("proposal construction failed"),
        "unexpected operational error: {message}"
    );
}

#[test]
fn invalid_scenario_remains_operational_error_under_observation_mode() {
    let scenario = Scenario::OrchardPayment(OrchardPaymentScenario {
        payment_zat: 120_000,
        ..OrchardPaymentScenario::demo()
    });
    let output = execute(&["--wallet-spend-policy", "orchard-only"], &scenario);
    assert!(!output.status.success());

    let RunnerResponse::Error { message } = response(&output) else {
        panic!("invalid scenario cannot be a wallet policy rejection");
    };
    assert!(message.contains("leave room for transaction fees"));
}

#[test]
fn policy_observation_keeps_the_exact_unrestricted_diagnostic_snapshot() {
    use shieldspec_zcash_runner::ShieldedPoolChoice;

    for funding_pool in [ShieldedPoolChoice::Orchard, ShieldedPoolChoice::Sapling] {
        let scenario = Scenario::OrchardPayment(OrchardPaymentScenario {
            funding_pool,
            ..OrchardPaymentScenario::demo()
        });
        let legacy_output = execute(&[], &scenario);
        let observed_output = execute(&["--wallet-spend-policy", "orchard-only"], &scenario);
        assert!(legacy_output.status.success(), "{legacy_output:?}");
        assert!(observed_output.status.success(), "{observed_output:?}");

        let RunnerResponse::Ok {
            snapshot: unrestricted,
        } = response(&legacy_output)
        else {
            panic!("default execution must return a snapshot");
        };
        let RunnerResponse::Observed {
            snapshot: diagnostic,
            ..
        } = response(&observed_output)
        else {
            panic!("opt-in execution must return a real wallet decision");
        };
        assert_eq!(
            diagnostic, unrestricted,
            "observing a policy must not rewrite the SDK-generated reference snapshot"
        );
    }
}
