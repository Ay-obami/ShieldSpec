use shieldspec_core::{
    assess, compare, validate_comparison_context, validate_snapshot, ComparisonContextError,
    InputFact, InputOrigin, OutputFact, OutputRole, Policy, Pool, PriorStepOutput,
    ProposalSnapshot, Severity, SnapshotValidationError, StepSnapshot,
};

fn proposal(steps: Vec<StepSnapshot>) -> ProposalSnapshot {
    ProposalSnapshot {
        schema_version: 1,
        network: "testnet".to_owned(),
        steps,
    }
}

fn step(inputs: Vec<Pool>, outputs: Vec<(Pool, OutputRole)>) -> StepSnapshot {
    StepSnapshot {
        index: 0,
        inputs: inputs
            .into_iter()
            .map(|pool| InputFact {
                pool,
                value_zat: 50_000,
                origin: Default::default(),
            })
            .collect(),
        outputs: outputs
            .into_iter()
            .map(|(pool, role)| OutputFact {
                pool,
                role,
                value_zat: 40_000,
            })
            .collect(),
        fee_zat: 10_000,
    }
}

fn finding<'a>(
    assessment: &'a shieldspec_core::Assessment,
    rule_id: &str,
) -> &'a shieldspec_core::Finding {
    assessment
        .findings
        .iter()
        .find(|finding| finding.rule_id == rule_id)
        .unwrap_or_else(|| panic!("missing finding {rule_id}"))
}

#[test]
fn standard_flags_transparent_recipient_as_policy_failing_warning() {
    let snapshot = proposal(vec![step(
        vec![Pool::Orchard],
        vec![(Pool::Transparent, OutputRole::Recipient)],
    )]);

    let assessment = assess(&snapshot, &Policy::standard());

    assert_eq!(
        finding(&assessment, "SS001_TRANSPARENT_RECIPIENT").severity,
        Severity::Warning
    );
    assert!(!assessment.passes);
}

#[test]
fn standard_flags_transparent_funding_as_policy_failing_warning() {
    let snapshot = proposal(vec![step(
        vec![Pool::Transparent],
        vec![(Pool::Orchard, OutputRole::Recipient)],
    )]);

    let assessment = assess(&snapshot, &Policy::standard());

    assert_eq!(
        finding(&assessment, "SS002_TRANSPARENT_FUNDING").severity,
        Severity::Warning
    );
    assert!(!assessment.passes);
}

#[test]
fn transparent_change_is_an_error_under_standard_policy() {
    let snapshot = proposal(vec![step(
        vec![Pool::Orchard],
        vec![(Pool::Transparent, OutputRole::Change)],
    )]);

    let assessment = assess(&snapshot, &Policy::standard());

    assert_eq!(
        finding(&assessment, "SS003_TRANSPARENT_CHANGE").severity,
        Severity::Error
    );
    assert!(!assessment.passes);
}

#[test]
fn unknown_input_and_output_routes_fail_closed() {
    let snapshot = proposal(vec![step(
        vec![Pool::Unknown],
        vec![(Pool::Unknown, OutputRole::Other)],
    )]);

    let assessment = assess(&snapshot, &Policy::standard());

    let unknowns: Vec<_> = assessment
        .findings
        .iter()
        .filter(|finding| finding.rule_id == "SS004_UNKNOWN_ROUTE")
        .collect();
    assert_eq!(unknowns.len(), 2);
    assert!(unknowns
        .iter()
        .all(|finding| finding.severity == Severity::Warning));
    assert!(!assessment.passes);
}

#[test]
fn crossing_between_shielded_and_transparent_pools_is_a_privacy_boundary() {
    let snapshot = proposal(vec![step(
        vec![Pool::Sapling],
        vec![(Pool::Transparent, OutputRole::Other)],
    )]);

    let assessment = assess(&snapshot, &Policy::standard());

    assert_eq!(
        finding(&assessment, "SS005_PRIVACY_BOUNDARY").severity,
        Severity::Warning
    );
    assert!(!assessment.passes);
}

#[test]
fn shielded_pool_migration_does_not_cross_a_privacy_boundary() {
    let snapshot = proposal(vec![step(
        vec![Pool::Sapling],
        vec![(Pool::Orchard, OutputRole::Other)],
    )]);

    let assessment = assess(&snapshot, &Policy::standard());

    assert!(assessment.findings.is_empty());
    assert!(assessment.passes);
}

#[test]
fn ephemeral_prior_step_bridge_is_not_treated_as_wallet_transparent_funding_or_boundary() {
    let snapshot = proposal(vec![
        StepSnapshot {
            index: 0,
            inputs: vec![InputFact {
                pool: Pool::Orchard,
                value_zat: 100,
                origin: InputOrigin::Wallet,
            }],
            outputs: vec![OutputFact {
                pool: Pool::Transparent,
                role: OutputRole::Ephemeral,
                value_zat: 90,
            }],
            fee_zat: 10,
        },
        StepSnapshot {
            index: 1,
            inputs: vec![InputFact {
                pool: Pool::Transparent,
                value_zat: 90,
                origin: InputOrigin::PriorStep {
                    step_index: 0,
                    output: PriorStepOutput::Change(0),
                },
            }],
            outputs: vec![OutputFact {
                pool: Pool::Orchard,
                role: OutputRole::Recipient,
                value_zat: 80,
            }],
            fee_zat: 10,
        },
    ]);

    let assessment = assess(&snapshot, &Policy::standard());

    assert!(
        assessment.passes,
        "internal ephemeral bridge should be privacy-neutral: {:?}",
        assessment.findings
    );
    assert!(assessment
        .findings
        .iter()
        .all(|finding| finding.rule_id != "SS002_TRANSPARENT_FUNDING"));
    assert!(assessment
        .findings
        .iter()
        .all(|finding| finding.rule_id != "SS005_PRIVACY_BOUNDARY"));
}

#[test]
fn strict_upgrades_every_exposure_to_error() {
    let snapshot = proposal(vec![step(
        vec![Pool::Transparent, Pool::Unknown],
        vec![
            (Pool::Transparent, OutputRole::Recipient),
            (Pool::Transparent, OutputRole::Change),
            (Pool::Unknown, OutputRole::Other),
            (Pool::Orchard, OutputRole::Other),
        ],
    )]);

    let assessment = assess(&snapshot, &Policy::strict());

    assert!(!assessment.findings.is_empty());
    assert!(assessment
        .findings
        .iter()
        .all(|finding| finding.severity == Severity::Error));
    assert!(!assessment.passes);
}

#[test]
fn a_leak_in_a_later_step_is_reported_with_that_step_index() {
    let mut safe = step(
        vec![Pool::Orchard],
        vec![(Pool::Orchard, OutputRole::Recipient)],
    );
    safe.index = 0;
    let mut leaking = step(
        vec![Pool::Orchard],
        vec![(Pool::Transparent, OutputRole::Change)],
    );
    leaking.index = 7;

    let assessment = assess(&proposal(vec![safe, leaking]), &Policy::standard());

    assert_eq!(
        finding(&assessment, "SS003_TRANSPARENT_CHANGE").step_index,
        7
    );
}

#[test]
fn fingerprints_are_stable_across_value_only_changes_and_serialization() {
    let original = proposal(vec![step(
        vec![Pool::Transparent],
        vec![(Pool::Orchard, OutputRole::Recipient)],
    )]);
    let mut changed = original.clone();
    changed.steps[0].inputs[0].value_zat = 999_999;
    changed.steps[0].outputs[0].value_zat = 987_654;
    changed.steps[0].fee_zat = 12_345;
    let roundtripped: ProposalSnapshot =
        serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();

    let fingerprint = finding(
        &assess(&original, &Policy::standard()),
        "SS002_TRANSPARENT_FUNDING",
    )
    .fingerprint
    .clone();
    assert_eq!(
        fingerprint,
        finding(
            &assess(&changed, &Policy::standard()),
            "SS002_TRANSPARENT_FUNDING"
        )
        .fingerprint
    );
    assert_eq!(
        fingerprint,
        finding(
            &assess(&roundtripped, &Policy::standard()),
            "SS002_TRANSPARENT_FUNDING"
        )
        .fingerprint
    );
}

#[test]
fn structural_positions_produce_distinct_fingerprints() {
    let snapshot = proposal(vec![step(
        vec![Pool::Transparent, Pool::Transparent],
        vec![(Pool::Orchard, OutputRole::Recipient)],
    )]);

    let assessment = assess(&snapshot, &Policy::standard());
    let fingerprints: Vec<_> = assessment
        .findings
        .iter()
        .filter(|finding| finding.rule_id == "SS002_TRANSPARENT_FUNDING")
        .map(|finding| &finding.fingerprint)
        .collect();

    assert_eq!(fingerprints.len(), 2);
    assert_ne!(fingerprints[0], fingerprints[1]);
}

#[test]
fn compare_reports_only_new_policy_failing_findings() {
    let baseline = proposal(vec![step(
        vec![Pool::Transparent],
        vec![(Pool::Orchard, OutputRole::Recipient)],
    )]);
    let mut candidate = baseline.clone();
    candidate.steps[0].outputs.push(OutputFact {
        pool: Pool::Transparent,
        role: OutputRole::Change,
        value_zat: 1_000,
    });

    let report = compare(&baseline, &candidate, &Policy::standard());

    assert!(!report.passes);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS003_TRANSPARENT_CHANGE"));
    assert!(!report
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS002_TRANSPARENT_FUNDING"));
}

#[test]
fn value_only_changes_do_not_regress() {
    let baseline = proposal(vec![step(
        vec![Pool::Transparent],
        vec![(Pool::Orchard, OutputRole::Recipient)],
    )]);
    let mut candidate = baseline.clone();
    candidate.steps[0].inputs[0].value_zat += 1;
    candidate.steps[0].outputs[0].value_zat += 1;

    let report = compare(&baseline, &candidate, &Policy::standard());

    assert!(report.passes);
    assert!(report.findings.is_empty());
}

#[test]
fn adding_the_same_kind_of_exposure_at_a_new_position_regresses() {
    let baseline = proposal(vec![step(
        vec![Pool::Transparent],
        vec![(Pool::Orchard, OutputRole::Recipient)],
    )]);
    let mut candidate = baseline.clone();
    candidate.steps[0].inputs.push(InputFact {
        pool: Pool::Transparent,
        value_zat: 1,
        origin: Default::default(),
    });

    let report = compare(&baseline, &candidate, &Policy::standard());

    assert!(!report.passes);
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|finding| finding.rule_id == "SS002_TRANSPARENT_FUNDING")
            .count(),
        1
    );
}

#[test]
fn serde_uses_stable_snake_case_model_names() {
    let snapshot = proposal(vec![step(
        vec![Pool::Ironwood],
        vec![(Pool::Orchard, OutputRole::Recipient)],
    )]);

    let json = serde_json::to_value(snapshot).unwrap();

    assert_eq!(json["steps"][0]["inputs"][0]["pool"], "ironwood");
    assert_eq!(json["steps"][0]["outputs"][0]["role"], "recipient");
}

#[test]
fn ss005_uses_net_directional_value_flow_and_ignores_fee_consumption() {
    let no_crossing = proposal(vec![StepSnapshot {
        index: 0,
        inputs: vec![
            InputFact {
                pool: Pool::Transparent,
                value_zat: 30,
                origin: Default::default(),
            },
            InputFact {
                pool: Pool::Orchard,
                value_zat: 70,
                origin: Default::default(),
            },
        ],
        outputs: vec![
            OutputFact {
                pool: Pool::Transparent,
                role: OutputRole::Other,
                value_zat: 30,
            },
            OutputFact {
                pool: Pool::Orchard,
                role: OutputRole::Other,
                value_zat: 60,
            },
        ],
        fee_zat: 10,
    }]);

    let assessment = assess(&no_crossing, &Policy::standard());
    assert!(!assessment
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS005_PRIVACY_BOUNDARY"));
}

#[test]
fn ss005_starts_and_stops_with_net_flow_but_amount_changes_keep_its_fingerprint() {
    let snapshot = |shielded_output| {
        proposal(vec![StepSnapshot {
            index: 0,
            inputs: vec![InputFact {
                pool: Pool::Transparent,
                value_zat: 100,
                origin: Default::default(),
            }],
            outputs: vec![
                OutputFact {
                    pool: Pool::Transparent,
                    role: OutputRole::Other,
                    value_zat: 90 - shielded_output,
                },
                OutputFact {
                    pool: Pool::Orchard,
                    role: OutputRole::Other,
                    value_zat: shielded_output,
                },
            ],
            fee_zat: 10,
        }])
    };

    let stopped = assess(&snapshot(0), &Policy::standard());
    let started = assess(&snapshot(20), &Policy::standard());
    let changed = assess(&snapshot(40), &Policy::standard());
    assert!(stopped
        .findings
        .iter()
        .all(|finding| finding.rule_id != "SS005_PRIVACY_BOUNDARY"));
    let started = finding(&started, "SS005_PRIVACY_BOUNDARY");
    let changed = finding(&changed, "SS005_PRIVACY_BOUNDARY");
    assert_eq!(started.evidence["direction"], "transparent_to_shielded");
    assert_eq!(started.fingerprint, changed.fingerprint);
}

#[test]
fn ss005_distinguishes_flow_directions_structurally() {
    let transparent_to_shielded = proposal(vec![StepSnapshot {
        index: 0,
        inputs: vec![InputFact {
            pool: Pool::Transparent,
            value_zat: 50,
            origin: Default::default(),
        }],
        outputs: vec![OutputFact {
            pool: Pool::Sapling,
            role: OutputRole::Other,
            value_zat: 40,
        }],
        fee_zat: 10,
    }]);
    let shielded_to_transparent = proposal(vec![StepSnapshot {
        index: 0,
        inputs: vec![InputFact {
            pool: Pool::Orchard,
            value_zat: 50,
            origin: Default::default(),
        }],
        outputs: vec![OutputFact {
            pool: Pool::Transparent,
            role: OutputRole::Other,
            value_zat: 40,
        }],
        fee_zat: 10,
    }]);

    let left = finding(
        &assess(&transparent_to_shielded, &Policy::standard()),
        "SS005_PRIVACY_BOUNDARY",
    )
    .fingerprint
    .clone();
    let right = finding(
        &assess(&shielded_to_transparent, &Policy::standard()),
        "SS005_PRIVACY_BOUNDARY",
    )
    .fingerprint
    .clone();
    assert_ne!(left, right);
}

#[test]
fn reordering_inputs_and_outputs_does_not_create_regressions() {
    let baseline = proposal(vec![StepSnapshot {
        index: 0,
        inputs: vec![
            InputFact {
                pool: Pool::Transparent,
                value_zat: 40,
                origin: Default::default(),
            },
            InputFact {
                pool: Pool::Orchard,
                value_zat: 60,
                origin: Default::default(),
            },
        ],
        outputs: vec![
            OutputFact {
                pool: Pool::Transparent,
                role: OutputRole::Recipient,
                value_zat: 25,
            },
            OutputFact {
                pool: Pool::Orchard,
                role: OutputRole::Change,
                value_zat: 65,
            },
        ],
        fee_zat: 10,
    }]);
    let mut candidate = baseline.clone();
    candidate.steps[0].inputs.reverse();
    candidate.steps[0].outputs.reverse();

    let report = compare(&baseline, &candidate, &Policy::standard());
    assert!(report.passes, "{:?}", report.findings);
}

#[test]
fn adding_a_second_structurally_equivalent_exposure_is_still_a_regression() {
    let baseline = proposal(vec![StepSnapshot {
        index: 0,
        inputs: vec![InputFact {
            pool: Pool::Transparent,
            value_zat: 100,
            origin: Default::default(),
        }],
        outputs: vec![OutputFact {
            pool: Pool::Orchard,
            role: OutputRole::Recipient,
            value_zat: 90,
        }],
        fee_zat: 10,
    }]);
    let mut candidate = baseline.clone();
    candidate.steps[0].inputs = vec![
        InputFact {
            pool: Pool::Transparent,
            value_zat: 40,
            origin: Default::default(),
        },
        InputFact {
            pool: Pool::Transparent,
            value_zat: 60,
            origin: Default::default(),
        },
    ];

    let report = compare(&baseline, &candidate, &Policy::standard());
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|finding| finding.rule_id == "SS002_TRANSPARENT_FUNDING")
            .count(),
        1
    );
}

#[test]
fn reordering_duplicate_structural_identities_keeps_fingerprints_stable() {
    let baseline = proposal(vec![StepSnapshot {
        index: 0,
        inputs: vec![
            InputFact {
                pool: Pool::Transparent,
                value_zat: 40,
                origin: Default::default(),
            },
            InputFact {
                pool: Pool::Transparent,
                value_zat: 60,
                origin: Default::default(),
            },
        ],
        outputs: vec![
            OutputFact {
                pool: Pool::Transparent,
                role: OutputRole::Recipient,
                value_zat: 25,
            },
            OutputFact {
                pool: Pool::Transparent,
                role: OutputRole::Recipient,
                value_zat: 65,
            },
        ],
        fee_zat: 10,
    }]);
    let mut candidate = baseline.clone();
    candidate.steps[0].inputs.reverse();
    candidate.steps[0].outputs.reverse();

    assert!(compare(&baseline, &candidate, &Policy::standard()).passes);
}

#[test]
fn adding_a_second_equivalent_output_exposure_is_a_regression() {
    let baseline = proposal(vec![StepSnapshot {
        index: 0,
        inputs: vec![InputFact {
            pool: Pool::Orchard,
            value_zat: 100,
            origin: Default::default(),
        }],
        outputs: vec![OutputFact {
            pool: Pool::Transparent,
            role: OutputRole::Recipient,
            value_zat: 90,
        }],
        fee_zat: 10,
    }]);
    let mut candidate = baseline.clone();
    candidate.steps[0].outputs = vec![
        OutputFact {
            pool: Pool::Transparent,
            role: OutputRole::Recipient,
            value_zat: 40,
        },
        OutputFact {
            pool: Pool::Transparent,
            role: OutputRole::Recipient,
            value_zat: 50,
        },
    ];

    let report = compare(&baseline, &candidate, &Policy::standard());
    assert_eq!(
        report
            .findings
            .iter()
            .filter(|finding| finding.rule_id == "SS001_TRANSPARENT_RECIPIENT")
            .count(),
        1
    );
}

#[test]
fn snapshot_validation_rejects_empty_bad_indexes_missing_facts_and_bad_balance() {
    assert_eq!(
        validate_snapshot(&proposal(vec![])),
        Err(SnapshotValidationError::NoSteps)
    );

    let bad_indexes = proposal(vec![
        StepSnapshot {
            index: 0,
            inputs: vec![InputFact {
                pool: Pool::Orchard,
                value_zat: 10,
                origin: Default::default(),
            }],
            outputs: vec![OutputFact {
                pool: Pool::Orchard,
                role: OutputRole::Other,
                value_zat: 9,
            }],
            fee_zat: 1,
        },
        StepSnapshot {
            index: 0,
            inputs: vec![InputFact {
                pool: Pool::Orchard,
                value_zat: 10,
                origin: Default::default(),
            }],
            outputs: vec![OutputFact {
                pool: Pool::Orchard,
                role: OutputRole::Other,
                value_zat: 9,
            }],
            fee_zat: 1,
        },
    ]);
    assert!(matches!(
        validate_snapshot(&bad_indexes),
        Err(SnapshotValidationError::InvalidStepIndex { .. })
    ));

    let mut missing_input = bad_indexes.steps[0].clone();
    missing_input.inputs.clear();
    assert_eq!(
        validate_snapshot(&proposal(vec![missing_input])),
        Err(SnapshotValidationError::NoInputs { step_index: 0 })
    );

    let mut missing_output = bad_indexes.steps[0].clone();
    missing_output.outputs.clear();
    assert_eq!(
        validate_snapshot(&proposal(vec![missing_output])),
        Err(SnapshotValidationError::NoOutputs { step_index: 0 })
    );

    let mut unbalanced = bad_indexes.steps[0].clone();
    unbalanced.outputs[0].value_zat = 8;
    assert!(matches!(
        validate_snapshot(&proposal(vec![unbalanced])),
        Err(SnapshotValidationError::UnbalancedStep { step_index: 0, .. })
    ));
}

#[test]
fn snapshot_validation_checks_schema_network_and_comparison_context() {
    let valid = proposal(vec![StepSnapshot {
        index: 0,
        inputs: vec![InputFact {
            pool: Pool::Orchard,
            value_zat: 10,
            origin: Default::default(),
        }],
        outputs: vec![OutputFact {
            pool: Pool::Orchard,
            role: OutputRole::Other,
            value_zat: 9,
        }],
        fee_zat: 1,
    }]);
    assert_eq!(validate_snapshot(&valid), Ok(()));

    let mut bad_schema = valid.clone();
    bad_schema.schema_version = 2;
    assert_eq!(
        validate_snapshot(&bad_schema),
        Err(SnapshotValidationError::UnsupportedSchemaVersion { actual: 2 })
    );
    let mut empty_network = valid.clone();
    empty_network.network.clear();
    assert_eq!(
        validate_snapshot(&empty_network),
        Err(SnapshotValidationError::EmptyNetwork)
    );

    let mut other_network = valid.clone();
    other_network.network = "mainnet".to_owned();
    assert!(matches!(
        validate_comparison_context(&valid, &other_network),
        Err(ComparisonContextError::NetworkMismatch { .. })
    ));
    let mut other_schema = valid.clone();
    other_schema.schema_version = 2;
    assert!(matches!(
        validate_comparison_context(&valid, &other_schema),
        Err(ComparisonContextError::SchemaVersionMismatch { .. })
    ));
}

#[test]
fn snapshot_models_deny_unknown_json_fields() {
    let base = serde_json::json!({
        "schema_version": 1,
        "network": "testnet",
        "steps": [{
            "index": 0,
            "inputs": [{"pool": "orchard", "value_zat": 10}],
            "outputs": [{"pool": "orchard", "role": "other", "value_zat": 9}],
            "fee_zat": 1
        }]
    });
    let mut cases = Vec::new();
    let mut snapshot = base.clone();
    snapshot["extra"] = serde_json::json!(true);
    cases.push(snapshot);
    let mut step = base.clone();
    step["steps"][0]["extra"] = serde_json::json!(true);
    cases.push(step);
    let mut input = base.clone();
    input["steps"][0]["inputs"][0]["extra"] = serde_json::json!(true);
    cases.push(input);
    let mut output = base;
    output["steps"][0]["outputs"][0]["extra"] = serde_json::json!(true);
    cases.push(output);

    for json in cases {
        assert!(serde_json::from_value::<ProposalSnapshot>(json).is_err());
    }
}

#[test]
fn public_assess_fails_closed_for_invalid_snapshots() {
    let invalid = proposal(vec![]);

    let assessment = assess(&invalid, &Policy::standard());

    assert!(!assessment.passes);
    assert!(assessment
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS000_INVALID_SNAPSHOT"));
}

#[test]
fn public_compare_fails_closed_for_invalid_snapshots_and_contexts() {
    let valid = proposal(vec![step(
        vec![Pool::Orchard],
        vec![(Pool::Orchard, OutputRole::Recipient)],
    )]);
    let mut invalid = valid.clone();
    invalid.steps.clear();

    let invalid_report = compare(&valid, &invalid, &Policy::standard());
    assert!(!invalid_report.passes);
    assert!(invalid_report
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS000_INVALID_SNAPSHOT"));

    let mut other_network = valid.clone();
    other_network.network = "mainnet".to_owned();
    let context_report = compare(&valid, &other_network, &Policy::standard());
    assert!(!context_report.passes);
    assert!(context_report
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS000_INVALID_COMPARISON"));
}
