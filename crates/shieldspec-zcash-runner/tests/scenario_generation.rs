use shieldspec_core::{assess, Policy, Pool};
use shieldspec_zcash_runner::{
    run_scenario, shrink_scenario, Discovery, GeneratedCase, Nu63Activation,
    Nu63OrchardPaymentScenario, OrchardPaymentScenario, Scenario, ScenarioGenerator,
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
    assert!(cases.iter().any(|case| match &case.scenario {
        Scenario::OrchardPayment(scenario) => scenario.funding_pool == ShieldedPoolChoice::Sapling,
        Scenario::TexPayment(scenario) => scenario.funding_pool == ShieldedPoolChoice::Sapling,
        Scenario::Nu63OrchardPayment(_) => unreachable!("legacy generator emits legacy cases"),
    }));
    assert!(cases.iter().any(|case| match &case.scenario {
        Scenario::OrchardPayment(scenario) => scenario.funding_pool != scenario.change_pool,
        Scenario::TexPayment(scenario) => scenario.funding_pool != scenario.change_pool,
        Scenario::Nu63OrchardPayment(_) => unreachable!("legacy generator emits legacy cases"),
    }));
    assert!(cases.iter().any(|case| match &case.scenario {
        Scenario::OrchardPayment(scenario) => scenario.confirmations > 1,
        Scenario::TexPayment(scenario) => scenario.confirmations > 1,
        Scenario::Nu63OrchardPayment(_) => unreachable!("legacy generator emits legacy cases"),
    }));
    assert!(cases.iter().any(|case| match &case.scenario {
        Scenario::OrchardPayment(scenario) => scenario.note_count > 1,
        Scenario::TexPayment(scenario) => scenario.note_count > 1,
        Scenario::Nu63OrchardPayment(_) => unreachable!("legacy generator emits legacy cases"),
    }));
    assert!(cases.iter().any(|case| match &case.scenario {
        Scenario::OrchardPayment(scenario) => scenario.empty_blocks > 0,
        Scenario::TexPayment(scenario) => scenario.empty_blocks > 0,
        Scenario::Nu63OrchardPayment(_) => unreachable!("legacy generator emits legacy cases"),
    }));

    for case in cases {
        let first = run_scenario(&case.scenario).expect("generated scenario must be executable");
        let second = run_scenario(&case.scenario).expect("generated scenario must replay");
        assert_eq!(first, second, "real proposal replay must be deterministic");

        let assessment = assess(&first, &Policy::standard());
        match &case.scenario {
            Scenario::OrchardPayment(scenario) => {
                // Actual proposal routing is authoritative: the SDK may keep
                // change in Orchard despite a Sapling change-pool preference.
                // This payment always has an Orchard recipient; any observed
                // Sapling input or output therefore crosses shielded pools.
                let crosses_pools = first.steps.iter().any(|step| {
                    step.inputs.iter().any(|input| input.pool == Pool::Sapling)
                        || step
                            .outputs
                            .iter()
                            .any(|output| output.pool == Pool::Sapling)
                });
                assert_eq!(
                    assessment
                        .findings
                        .iter()
                        .any(|finding| finding.rule_id == "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"),
                    crosses_pools,
                    "cross-pool routing mismatch: scenario={scenario:?}, snapshot={first:?}, findings={:?}",
                    assessment.findings
                );
                assert!(
                    assessment.findings.iter().all(|finding| {
                        finding.rule_id == "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"
                    }),
                    "ordinary shielded payments must not have unrelated findings: {:?}",
                    assessment.findings
                );
            }
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
            Scenario::Nu63OrchardPayment(_) => {
                unreachable!("legacy generator emits legacy cases")
            }
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

fn nu63_payment(funding_pool: ShieldedPoolChoice, activation: Nu63Activation) -> Scenario {
    let mut payment = OrchardPaymentScenario::demo();
    payment.funding_zat = 200_000;
    payment.payment_zat = 100_000;
    payment.confirmations = 2;
    payment.empty_blocks = 1;
    payment.funding_pool = funding_pool;
    payment.change_pool = funding_pool;

    Scenario::Nu63OrchardPayment(Nu63OrchardPaymentScenario {
        payment,
        activation,
    })
}

#[test]
fn real_nu63_boundary_routes_orchard_receiver_into_ironwood() {
    let before = run_scenario(&nu63_payment(
        ShieldedPoolChoice::Orchard,
        Nu63Activation::Before,
    ))
    .expect("pre-activation real proposal");
    let after = run_scenario(&nu63_payment(
        ShieldedPoolChoice::Orchard,
        Nu63Activation::At,
    ))
    .expect("post-activation real proposal");

    for snapshot in [&before, &after] {
        assert_eq!(snapshot.steps.len(), 1);
        assert!(snapshot.steps[0]
            .inputs
            .iter()
            .all(|input| input.pool == Pool::Orchard));
        assert!(snapshot.steps[0].fee_zat > 0);
    }

    assert!(before.steps[0].outputs.iter().any(|output| {
        output.pool == Pool::Orchard
            && output.role == shieldspec_core::OutputRole::Recipient
            && output.value_zat == 100_000
    }));
    assert!(!before.steps[0]
        .outputs
        .iter()
        .any(|output| output.pool == Pool::Ironwood));

    assert!(after.steps[0].outputs.iter().any(|output| {
        output.pool == Pool::Ironwood
            && output.role == shieldspec_core::OutputRole::Recipient
            && output.value_zat == 100_000
    }));
    let before_assessment = assess(&before, &Policy::standard());
    assert!(before_assessment
        .findings
        .iter()
        .all(|finding| finding.rule_id != "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"));
    let after_assessment = assess(&after, &Policy::standard());
    let crossing = after_assessment
        .findings
        .iter()
        .find(|finding| finding.rule_id == "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE")
        .expect("actual proposal crosses Orchard to Ironwood");
    assert_eq!(crossing.evidence["source_pool"], "orchard");
    assert_eq!(crossing.evidence["destination_pool"], "ironwood");
    assert_eq!(crossing.evidence["crossing_value_zat"], "100000");
}

#[test]
fn real_sapling_funding_selects_ironwood_after_nu63() {
    let snapshot = run_scenario(&nu63_payment(
        ShieldedPoolChoice::Sapling,
        Nu63Activation::At,
    ))
    .expect("Sapling-funded post-activation proposal");
    assert!(snapshot.steps[0]
        .inputs
        .iter()
        .all(|input| input.pool == Pool::Sapling));
    assert!(snapshot.steps[0].outputs.iter().any(|output| {
        output.pool == Pool::Ironwood
            && output.role == shieldspec_core::OutputRole::Recipient
            && output.value_zat == 100_000
    }));
    assert!(assess(&snapshot, &Policy::standard())
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"));
}

#[test]
fn generated_nu63_campaign_covers_both_funding_pools_and_activation_states() {
    for (ordinal, activation, funding_pool) in [
        (0, Nu63Activation::At, ShieldedPoolChoice::Orchard),
        (1, Nu63Activation::Before, ShieldedPoolChoice::Orchard),
        (2, Nu63Activation::At, ShieldedPoolChoice::Sapling),
        (3, Nu63Activation::Before, ShieldedPoolChoice::Sapling),
    ] {
        let scenario = ScenarioGenerator::nu63_scenario_at(0x5eed_6300, ordinal);
        let Scenario::Nu63OrchardPayment(payment) = &scenario else {
            panic!("expected activation-aware proposal");
        };
        assert_eq!(payment.activation, activation, "ordinal {ordinal}");
        assert_eq!(
            payment.payment.funding_pool, funding_pool,
            "ordinal {ordinal}"
        );
        assert_eq!(
            payment.payment.change_pool, funding_pool,
            "ordinal {ordinal}"
        );
    }

    let sapling_to_ironwood = ScenarioGenerator::nu63_scenario_at(0x5eed_6300, 2);
    let snapshot = run_scenario(&sapling_to_ironwood).expect("real generated proposal");
    assert!(snapshot.steps[0]
        .inputs
        .iter()
        .any(|input| input.pool == Pool::Sapling));
    assert!(snapshot.steps[0]
        .outputs
        .iter()
        .any(|output| output.pool == Pool::Ironwood));
    assert!(assess(&snapshot, &Policy::standard())
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"));
}

#[test]
fn opt_in_nu63_generation_replays_and_shrinks_without_changing_activation() {
    let scenario = ScenarioGenerator::nu63_scenario_at(0x5eed_6300, 0);
    assert_eq!(
        scenario,
        ScenarioGenerator::nu63_scenario_at(0x5eed_6300, 0)
    );
    let serialized = serde_json::to_string(&scenario).expect("serialize");
    let replay: Scenario = serde_json::from_str(&serialized).expect("decode");
    assert_eq!(replay, scenario);
    assert_eq!(
        run_scenario(&scenario).expect("first"),
        run_scenario(&replay).expect("second")
    );

    let crossing = |candidate: &Scenario| {
        let Ok(snapshot) = run_scenario(candidate) else {
            return false;
        };
        assess(&snapshot, &Policy::standard())
            .findings
            .iter()
            .any(|finding| finding.rule_id == "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE")
    };
    assert!(
        crossing(&scenario),
        "ordinal zero chooses post-NU6.3 crossing"
    );
    let minimized = shrink_scenario(&scenario, crossing);
    assert!(crossing(&minimized));
    assert!(minimized.complexity() <= scenario.complexity());
    assert!(matches!(
        minimized,
        Scenario::Nu63OrchardPayment(Nu63OrchardPaymentScenario {
            activation: Nu63Activation::At,
            ..
        })
    ));
}

#[test]
fn nu63_generated_cases_replay_and_cover_the_eight_way_pool_matrix() {
    for seed in [0_u64, 0x5eed_6300, u64::MAX] {
        let mut covered = 0_u8;
        for ordinal in 0..32 {
            let case = ScenarioGenerator::nu63_case_at(seed, ordinal);
            assert_eq!((case.seed, case.ordinal), (seed, ordinal));
            assert_eq!(
                case,
                ScenarioGenerator::nu63_case_at(case.seed, case.ordinal),
                "NU6.3 seed + ordinal must reproduce the entire case"
            );
            assert_eq!(
                case.scenario,
                ScenarioGenerator::nu63_scenario_at(seed, ordinal),
                "the existing opt-in scenario entry point must retain its contract"
            );
            let json = serde_json::to_string(&case).expect("encode NU6.3 case");
            let replay: GeneratedCase = serde_json::from_str(&json).expect("decode NU6.3 case");
            assert_eq!(replay, case);

            let Scenario::Nu63OrchardPayment(payment) = &case.scenario else {
                panic!("NU6.3 campaign must use real activation-aware payments");
            };
            let bit = (u8::from(payment.activation == Nu63Activation::At) << 2)
                | (u8::from(payment.payment.funding_pool == ShieldedPoolChoice::Sapling) << 1)
                | u8::from(payment.payment.change_pool != payment.payment.funding_pool);
            covered |= 1 << bit;
            assert!((1..=8).contains(&payment.payment.note_count));
            assert!((1..=10).contains(&payment.payment.confirmations));
            assert!(payment.payment.payment_zat > 0);
            assert!(
                payment.payment.funding_zat
                    >= payment.payment.payment_zat + case.scenario.minimum_fee_margin_zat(),
                "generated cases must leave a realistic proposal fee margin"
            );
        }
        assert_eq!(
            covered, 0xff,
            "one seed must cover both activation states, both funding pools, and both change-pool preferences"
        );
    }
}

#[test]
fn nu63_eight_way_campaign_observes_only_real_shielded_value_flows() {
    for ordinal in 0..8 {
        let case = ScenarioGenerator::nu63_case_at(0x5eed_6300, ordinal);
        let Scenario::Nu63OrchardPayment(payment) = &case.scenario else {
            panic!("campaign must execute an activation-aware proposal");
        };
        let snapshot = run_scenario(&case.scenario).unwrap_or_else(|error| {
            panic!("seed={} ordinal={ordinal} must execute: {error}", case.seed)
        });
        assert_eq!(snapshot.steps.len(), 1, "ordinal={ordinal}");
        let step = &snapshot.steps[0];
        let expected_input = match payment.payment.funding_pool {
            ShieldedPoolChoice::Orchard => Pool::Orchard,
            ShieldedPoolChoice::Sapling => Pool::Sapling,
        };
        let expected_recipient = match payment.activation {
            Nu63Activation::Before => Pool::Orchard,
            Nu63Activation::At => Pool::Ironwood,
        };

        assert!(!step.inputs.is_empty(), "ordinal={ordinal}");
        assert!(
            step.inputs.iter().all(|input| input.pool == expected_input),
            "no synthetic Ironwood funding or transparent funding: ordinal={ordinal}, step={step:?}"
        );
        assert_eq!(
            step.outputs
                .iter()
                .filter(|output| output.role == shieldspec_core::OutputRole::Recipient)
                .count(),
            1,
            "one real recipient output per proposal: ordinal={ordinal}"
        );
        assert!(
            step.outputs.iter().any(|output| {
                output.role == shieldspec_core::OutputRole::Recipient
                    && output.pool == expected_recipient
                    && output.value_zat == payment.payment.payment_zat
            }),
            "SDK-chosen recipient pool/value: ordinal={ordinal}, step={step:?}"
        );
        assert!(
            step.outputs.iter().all(|output| {
                matches!(output.pool, Pool::Sapling | Pool::Orchard | Pool::Ironwood)
            }),
            "no transparent or unknown outputs: ordinal={ordinal}, step={step:?}"
        );

        let input_zat: u64 = step.inputs.iter().map(|input| input.value_zat).sum();
        let output_zat: u64 = step.outputs.iter().map(|output| output.value_zat).sum();
        assert!(
            step.fee_zat > 0,
            "SDK must specify a fee: ordinal={ordinal}"
        );
        assert_eq!(
            input_zat,
            output_zat + step.fee_zat,
            "real proposal must conserve value: ordinal={ordinal}"
        );
        let findings = assess(&snapshot, &Policy::standard()).findings;
        if expected_input != expected_recipient {
            assert!(
                findings.iter().any(|finding| {
                    finding.rule_id == "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"
                }),
                "the real cross-pool output must be reported: ordinal={ordinal}, findings={findings:?}"
            );
        }
        assert!(
            findings.iter().all(|finding| {
                !matches!(
                    finding.rule_id.as_str(),
                    "SS001_TRANSPARENT_RECIPIENT"
                        | "SS002_TRANSPARENT_FUNDING"
                        | "SS003_TRANSPARENT_CHANGE"
                        | "SS005_PRIVACY_BOUNDARY"
                )
            }),
            "shielded-only SDK output must not fabricate a transparent privacy boundary"
        );
    }
}

#[test]
fn nu63_pre_activation_sapling_crossing_shrinks_without_changing_its_cause() {
    let original = ScenarioGenerator::nu63_case_at(0x5eed_6300, 3);
    let still_sapling_to_orchard = |candidate: &Scenario| {
        if !matches!(
            candidate,
            Scenario::Nu63OrchardPayment(Nu63OrchardPaymentScenario {
                activation: Nu63Activation::Before,
                ..
            })
        ) {
            return false;
        }
        let Ok(snapshot) = run_scenario(candidate) else {
            return false;
        };
        assess(&snapshot, &Policy::standard())
            .findings
            .iter()
            .any(|finding| {
                finding.rule_id == "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"
                    && finding.evidence.get("source_pool").map(String::as_str) == Some("sapling")
                    && finding.evidence.get("destination_pool").map(String::as_str)
                        == Some("orchard")
            })
    };

    assert!(still_sapling_to_orchard(&original.scenario));
    let minimized = shrink_scenario(&original.scenario, still_sapling_to_orchard);
    assert!(still_sapling_to_orchard(&minimized));
    assert!(
        minimized.complexity() < original.scenario.complexity(),
        "real Sapling-to-Orchard finding must admit a smaller replay"
    );
    let replay: Scenario =
        serde_json::from_str(&serde_json::to_string(&minimized).expect("encode minimized case"))
            .expect("decode minimized case");
    assert_eq!(replay, minimized);
    assert_eq!(
        run_scenario(&replay).expect("minimized replay"),
        run_scenario(&minimized).expect("repeated minimized replay")
    );
    assert_eq!(
        shrink_scenario(&minimized, still_sapling_to_orchard),
        minimized,
        "shrinking is idempotent for the same real rule evidence"
    );
}
