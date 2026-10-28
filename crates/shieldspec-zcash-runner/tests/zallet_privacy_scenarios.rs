use shieldspec_zcash_runner::{
    shrink_scenario, GeneratedCase, Scenario, ScenarioGenerator, ZalletChangePool, ZalletInputPool,
    ZalletPrivacyProbeScenario,
};

#[test]
fn zallet_privacy_campaign_replays_three_distinct_policy_inputs() {
    for seed in [0, 2026, u64::MAX] {
        let first_three = (0..3)
            .map(|ordinal| ScenarioGenerator::zallet_privacy_case_at(seed, ordinal))
            .collect::<Vec<_>>();
        let expected_pools = [
            (ZalletInputPool::Ironwood, ZalletChangePool::Ironwood),
            (ZalletInputPool::Orchard, ZalletChangePool::Sapling),
            (ZalletInputPool::Ironwood, ZalletChangePool::Sapling),
        ];
        for (ordinal, (case, pools)) in first_three.iter().zip(expected_pools).enumerate() {
            assert_eq!((case.seed, case.ordinal), (seed, ordinal as u64));
            let Scenario::ZalletPrivacyProbe(probe) = &case.scenario else {
                panic!("Zallet campaign must emit a wallet policy probe");
            };
            assert_eq!((probe.input_pool, probe.change_pool), pools);
            assert!(probe.input_zat > probe.fee_zat && probe.fee_zat > 0);
            let encoded = serde_json::to_string(case).expect("serialize");
            assert_eq!(
                serde_json::from_str::<GeneratedCase>(&encoded).unwrap(),
                *case
            );
            assert_eq!(
                ScenarioGenerator::zallet_privacy_case_at(seed, ordinal as u64),
                *case
            );
        }
    }
}

#[test]
fn zallet_privacy_probe_wire_schema_is_strict_and_balanced() {
    let case = Scenario::ZalletPrivacyProbe(ZalletPrivacyProbeScenario {
        input_pool: ZalletInputPool::Ironwood,
        change_pool: ZalletChangePool::Sapling,
        input_zat: 20_000,
        fee_zat: 10_000,
    });
    let expected = serde_json::json!({
        "kind": "zallet_privacy_probe",
        "params": {
            "input_pool": "ironwood",
            "change_pool": "sapling",
            "input_zat": 20_000,
            "fee_zat": 10_000
        }
    });
    assert_eq!(serde_json::to_value(&case).unwrap(), expected);
    assert_eq!(serde_json::from_value::<Scenario>(expected).unwrap(), case);
    let unsupported_input = r#"{"kind":"zallet_privacy_probe","params":{"input_pool":"sapling","change_pool":"sapling","input_zat":20000,"fee_zat":10000}}"#;
    assert!(serde_json::from_str::<Scenario>(unsupported_input).is_err());
    let unknown_field = r#"{"kind":"zallet_privacy_probe","params":{"input_pool":"ironwood","change_pool":"sapling","input_zat":20000,"fee_zat":10000,"verdict":"accepted"}}"#;
    assert!(serde_json::from_str::<Scenario>(unknown_field).is_err());
    for (input_zat, fee_zat) in [
        (0u64, 0u64),
        (10, 0),
        (10, 10),
        (10, 11),
        (2_100_000_000_000_001, 1),
    ] {
        let malformed = serde_json::json!({
            "kind": "zallet_privacy_probe",
            "params": {
                "input_pool": "ironwood",
                "change_pool": "sapling",
                "input_zat": input_zat,
                "fee_zat": fee_zat,
            }
        });
        assert!(serde_json::from_value::<Scenario>(malformed).is_err());
    }
}

#[test]
fn zallet_privacy_shrinker_preserves_causal_pools_and_positive_change() {
    let original = Scenario::ZalletPrivacyProbe(ZalletPrivacyProbeScenario {
        input_pool: ZalletInputPool::Ironwood,
        change_pool: ZalletChangePool::Sapling,
        input_zat: 230_000,
        fee_zat: 10_000,
    });
    let meaningful = |candidate: &Scenario| match candidate {
        Scenario::ZalletPrivacyProbe(probe) => {
            probe.input_pool == ZalletInputPool::Ironwood
                && probe.change_pool == ZalletChangePool::Sapling
                && probe.fee_zat > 0
                && probe.input_zat > probe.fee_zat
        }
        _ => false,
    };
    let minimized = shrink_scenario(&original, meaningful);
    assert!(meaningful(&minimized));
    assert!(minimized.complexity() < original.complexity());
    assert_eq!(
        serde_json::from_str::<Scenario>(&serde_json::to_string(&minimized).unwrap()).unwrap(),
        minimized
    );
}
