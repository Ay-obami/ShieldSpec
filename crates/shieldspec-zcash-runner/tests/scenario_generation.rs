use shieldspec_core::{assess, Policy, Pool};
use shieldspec_zcash_runner::{
    run_scenario, shrink_scenario, Discovery, GeneratedCase, Scenario, ScenarioGenerator,
    ShieldedPoolChoice,
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
fn sapling_recipient_corpus_runs_real_cross_pool_proposals_and_replays() {
    let seed = 0x5a91_1eaf;
    let mut generator = ScenarioGenerator::new(seed);
    for ordinal in 0..4 {
        let scenario = generator.next_sapling_recipient_scenario();
        assert_eq!(
            scenario,
            ScenarioGenerator::sapling_recipient_scenario_at(seed, ordinal)
        );
        let Scenario::OrchardPayment(ref params) = scenario else {
            panic!("dedicated generator must produce a shielded payment");
        };
        assert_eq!(params.recipient_pool, ShieldedPoolChoice::Sapling);
        let snapshot = run_scenario(&scenario).expect("real Zcash wallet proposal");
        assert!(snapshot.steps[0].outputs.iter().any(|output| {
            output.pool == Pool::Sapling
                && output.role == shieldspec_core::OutputRole::Recipient
                && output.value_zat == params.payment_zat
        }));
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
    let cases = (0..16).map(|_| generator.next_case()).collect::<Vec<_>>();

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
    assert!(
        cases
            .iter()
            .any(|case| matches!(case.scenario, Scenario::UnifiedPayment(_))),
        "sample should include mixed Unified Address receiver selection"
    );
    assert!(cases.iter().any(|case| match &case.scenario {
        Scenario::OrchardPayment(scenario) => scenario.funding_pool == ShieldedPoolChoice::Sapling,
        Scenario::TexPayment(scenario) => scenario.funding_pool == ShieldedPoolChoice::Sapling,
        Scenario::UnifiedPayment(scenario) => scenario.funding_pool == ShieldedPoolChoice::Sapling,
        Scenario::TransparentFundedPayment(scenario) =>
            scenario.recipient_pool == ShieldedPoolChoice::Sapling,
    }));
    assert!(cases.iter().any(|case| match &case.scenario {
        Scenario::OrchardPayment(scenario) => scenario.funding_pool != scenario.change_pool,
        Scenario::TexPayment(scenario) => scenario.funding_pool != scenario.change_pool,
        Scenario::UnifiedPayment(scenario) => scenario.funding_pool != scenario.change_pool,
        Scenario::TransparentFundedPayment(scenario) =>
            scenario.recipient_pool != scenario.change_pool,
    }));
    assert!(cases.iter().any(|case| match &case.scenario {
        Scenario::OrchardPayment(scenario) => scenario.confirmations > 1,
        Scenario::TexPayment(scenario) => scenario.confirmations > 1,
        Scenario::UnifiedPayment(scenario) => scenario.confirmations > 1,
        Scenario::TransparentFundedPayment(scenario) => scenario.confirmations > 1,
    }));
    assert!(cases.iter().any(|case| match &case.scenario {
        Scenario::OrchardPayment(scenario) => scenario.note_count > 1,
        Scenario::TexPayment(scenario) => scenario.note_count > 1,
        Scenario::UnifiedPayment(scenario) => scenario.note_count > 1,
        Scenario::TransparentFundedPayment(scenario) => scenario.utxo_count > 1,
    }));
    assert!(cases.iter().any(|case| match &case.scenario {
        Scenario::OrchardPayment(scenario) => scenario.empty_blocks > 0,
        Scenario::TexPayment(scenario) => scenario.empty_blocks > 0,
        Scenario::UnifiedPayment(scenario) => scenario.empty_blocks > 0,
        Scenario::TransparentFundedPayment(_) => false,
    }));

    for case in cases {
        let first = run_scenario(&case.scenario).expect("generated scenario must be executable");
        let second = run_scenario(&case.scenario).expect("generated scenario must replay");
        assert_eq!(first, second, "real proposal replay must be deterministic");

        let assessment = assess(&first, &Policy::standard());
        match &case.scenario {
            Scenario::OrchardPayment(_) => assert!(
                assessment.passes,
                "ordinary shielded scenarios should stay clean: {:?}",
                assessment.findings
            ),
            Scenario::TexPayment(_) => {
                assert!(assessment
                    .findings
                    .iter()
                    .any(|finding| finding.rule_id == "SS001_TRANSPARENT_RECIPIENT"));
                assert!(assessment
                    .findings
                    .iter()
                    .any(|finding| finding.rule_id == "SS005_PRIVACY_BOUNDARY"));
                assert!(assessment
                    .findings
                    .iter()
                    .all(|finding| finding.rule_id != "SS002_TRANSPARENT_FUNDING"));
                assert!(assessment
                    .findings
                    .iter()
                    .all(|finding| finding.rule_id != "SS003_TRANSPARENT_CHANGE"));
            }
            Scenario::UnifiedPayment(_) => assert!(
                assessment.passes,
                "Unified Address with a supported shielded receiver should stay shielded: {:?}",
                assessment.findings
            ),
            Scenario::TransparentFundedPayment(_) => assert!(assessment
                .findings
                .iter()
                .any(|finding| finding.rule_id == "SS002_TRANSPARENT_FUNDING")),
        }
    }
}

#[test]
fn committed_known_tex_finding_replays_from_seed_and_stays_minimal() {
    let discovery: Discovery = serde_json::from_str(include_str!(
        "../fixtures/discoveries/tex-transparent-recipient.json"
    ))
    .expect("committed replay corpus fixture");

    let generated = ScenarioGenerator::case_at(discovery.seed, discovery.ordinal);
    assert_eq!(generated.scenario, discovery.original);
    assert!(has_transparent_recipient_failure(&discovery.original));

    let minimized = shrink_scenario(&discovery.original, has_transparent_recipient_failure);
    assert_eq!(minimized, discovery.minimized);
    assert!(has_transparent_recipient_failure(&discovery.minimized));
    assert!(
        discovery.minimized.complexity() < discovery.original.complexity(),
        "committed known-behavior case should remain a minimized counterexample"
    );

    let replayed = discovery
        .replay_minimized()
        .expect("replay minimized corpus case");
    assert!(assess(&replayed, &Policy::standard())
        .findings
        .iter()
        .any(|finding| finding.rule_id == discovery.rule_id));
}
