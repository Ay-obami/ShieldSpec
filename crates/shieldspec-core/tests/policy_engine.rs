use shieldspec_core::{
    assess, compare, InputFact, OutputFact, OutputRole, Policy, Pool, ProposalSnapshot, Severity,
    StepSnapshot,
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
