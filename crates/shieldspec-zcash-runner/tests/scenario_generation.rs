use shieldspec_core::{assess, Policy};
use shieldspec_zcash_runner::{
    run_scenario, shrink_scenario, GeneratedCase, Scenario, ScenarioGenerator, TexPaymentScenario,
};

fn has_transparent_recipient_failure(scenario: &Scenario) -> bool {
    let Ok(snapshot) = run_scenario(scenario) else {
        return false;
    };
    assess(&snapshot, &Policy::standard())
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS001_TRANSPARENT_RECIPIENT")
}

#[test]
fn seed_and_ordinal_replay_the_exact_generated_case() {
    let seed = 0x5eed_cafe_u64;
    let mut generator = ScenarioGenerator::new(seed);
    let generated = (0..8).map(|_| generator.next_case()).collect::<Vec<_>>();

    for case in generated {
        assert_eq!(
            ScenarioGenerator::case_at(case.seed, case.ordinal),
            case,
            "seed + ordinal must reproduce the exact scenario"
        );
    }
}

#[test]
fn generated_case_json_is_a_stable_replay_contract() {
    let case = ScenarioGenerator::case_at(7, 3);
    let json = serde_json::to_string_pretty(&case).expect("serialize generated case");
    let decoded: GeneratedCase = serde_json::from_str(&json).expect("deserialize generated case");

    assert_eq!(decoded, case);
    assert_eq!(
        run_scenario(&decoded.scenario).expect("first replay"),
        run_scenario(&case.scenario).expect("second replay")
    );
}

#[test]
fn representative_generated_cases_execute_through_real_zcash_proposals() {
    let mut generator = ScenarioGenerator::new(0x0a11_ce55);
    let cases = (0..6).map(|_| generator.next_case()).collect::<Vec<_>>();

    assert!(
        cases
            .iter()
            .any(|case| matches!(case.scenario, Scenario::TexPayment(_))),
        "sample should include the privacy-sensitive ZIP 320 path"
    );
    assert!(
        cases
            .iter()
            .any(|case| matches!(case.scenario, Scenario::OrchardPayment(_))),
        "sample should include a shielded baseline path"
    );

    for case in cases {
        let first = run_scenario(&case.scenario).expect("generated scenario must be executable");
        let second = run_scenario(&case.scenario).expect("generated scenario must replay");
        assert_eq!(first, second, "real proposal replay must be deterministic");
    }
}

#[test]
fn real_privacy_failure_shrinks_to_a_smaller_reproducible_counterexample() {
    let original = Scenario::TexPayment(TexPaymentScenario {
        funding_zat: 100_000,
        payment_zat: 50_000,
        recipient_tag: 9,
        note_count: 3,
        empty_blocks: 2,
    });

    assert!(has_transparent_recipient_failure(&original));
    let minimized = shrink_scenario(&original, has_transparent_recipient_failure);
    println!(
        "{}",
        serde_json::to_string_pretty(&minimized).expect("serialize minimized scenario")
    );

    assert!(has_transparent_recipient_failure(&minimized));
    assert!(
        minimized.complexity() < original.complexity(),
        "shrinking should reduce the semantic scenario"
    );

    let Scenario::TexPayment(minimized) = minimized else {
        panic!("shrinking must preserve the scenario kind");
    };
    assert_eq!(minimized.note_count, 1);
    assert_eq!(minimized.empty_blocks, 0);
    assert_eq!(minimized.recipient_tag, 0);
}
