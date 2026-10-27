use shieldspec_core::{assess, Policy};
use shieldspec_zcash_runner::{run_scenario, shrink_scenario, Scenario, ScenarioGenerator};

#[test]
fn extended_generator_produces_replayable_transparent_funding() {
    let seed = 0x72ca_fe_u64;
    let mut generator = ScenarioGenerator::new_with_transparent_funding(seed);
    let cases = (0..48).map(|_| generator.next_case()).collect::<Vec<_>>();
    let funded = cases
        .iter()
        .find(|case| matches!(case.scenario, Scenario::TransparentFundedPayment(_)))
        .expect("transparent-funded scenarios must be generated");
    assert_eq!(
        *funded,
        ScenarioGenerator::case_at_with_transparent_funding(seed, funded.ordinal)
    );
    let snapshot = run_scenario(&funded.scenario).expect("real wallet generated proposal");
    assert!(assess(&snapshot, &Policy::standard())
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS002_TRANSPARENT_FUNDING"));
    let serialized = serde_json::to_string(&funded.scenario).unwrap();
    let replay: Scenario = serde_json::from_str(&serialized).unwrap();
    assert_eq!(snapshot, run_scenario(&replay).unwrap());

    let minimized = shrink_scenario(&funded.scenario, |candidate| {
        run_scenario(candidate)
            .ok()
            .map(|snapshot| {
                assess(&snapshot, &Policy::standard())
                    .findings
                    .iter()
                    .any(|finding| finding.rule_id == "SS002_TRANSPARENT_FUNDING")
            })
            .unwrap_or(false)
    });
    assert!(minimized.complexity() < funded.scenario.complexity());
    assert!(matches!(minimized, Scenario::TransparentFundedPayment(_)));
}

#[test]
fn cli_replays_extended_transparent_seed_by_ordinal() {
    let seed = 0x72ca_fe_u64;
    let mut generator = ScenarioGenerator::new_with_transparent_funding(seed);
    let ordinal = (0..48)
        .map(|_| generator.next_case())
        .find(|case| matches!(case.scenario, Scenario::TransparentFundedPayment(_)))
        .expect("extended generator transparent case")
        .ordinal;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_shieldspec-fuzz"))
        .args([
            "replay",
            "--seed",
            &seed.to_string(),
            "--ordinal",
            &ordinal.to_string(),
            "--transparent-funding",
            "--format",
            "json",
        ])
        .output()
        .expect("fuzzer replay executable");
    let body = String::from_utf8(output.stdout).expect("JSON stdout");
    assert!(body.contains("SS002_TRANSPARENT_FUNDING"), "{body}");
    assert!(body.contains("privacy_finding_reproduced"), "{body}");
}

#[test]
fn default_generator_keeps_legacy_replay_sequence() {
    let seed = 0x5eed_cafe_u64;
    assert_eq!(ScenarioGenerator::case_at(seed, 4), {
        let mut generator = ScenarioGenerator::new(seed);
        for _ in 0..4 {
            let _ = generator.next_case();
        }
        generator.next_case()
    });
}
