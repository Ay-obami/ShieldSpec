use shieldspec_core::{assess, OutputRole, Policy, Pool, ProposalSnapshot};
use shieldspec_testkit::{fixture, fixture_names, write_fixture, FixtureWriteError};
use std::collections::BTreeSet;

const EXPECTED_NAMES: &[&str] = &[
    "shielded-payment",
    "transparent-recipient",
    "transparent-funding",
    "transparent-change",
    "unified-selected-transparent",
    "multi-step-late-leak",
    "privacy-preserving-migration",
    "full-balance-migration",
];

fn rule_ids(snapshot: &ProposalSnapshot) -> BTreeSet<String> {
    assess(snapshot, &Policy::standard())
        .findings
        .into_iter()
        .map(|finding| finding.rule_id)
        .collect()
}

#[test]
fn fixture_names_are_complete_and_stably_ordered() {
    assert_eq!(fixture_names(), EXPECTED_NAMES);
}

#[test]
fn every_fixture_roundtrips_with_the_canonical_envelope() {
    for &name in fixture_names() {
        let snapshot = fixture(name).unwrap_or_else(|| panic!("missing fixture {name}"));
        assert_eq!(snapshot.schema_version, 1, "{name}");
        assert_eq!(snapshot.network, "test", "{name}");

        let encoded = serde_json::to_string(&snapshot).expect("fixture serializes");
        let decoded: ProposalSnapshot =
            serde_json::from_str(&encoded).expect("fixture deserializes");
        assert_eq!(decoded, snapshot, "{name}");
    }
}

#[test]
fn unknown_fixture_names_return_none() {
    assert!(fixture("not-a-fixture").is_none());
}

#[test]
fn write_fixture_writes_roundtrippable_json_and_rejects_unknown_names() {
    let path = std::env::temp_dir().join(format!(
        "shieldspec-testkit-fixture-{}.json",
        std::process::id()
    ));
    write_fixture("shielded-payment", &path).expect("write fixture");
    let written: ProposalSnapshot =
        serde_json::from_slice(&std::fs::read(&path).expect("read written fixture"))
            .expect("parse written fixture");
    std::fs::remove_file(path).expect("remove temporary fixture");

    assert_eq!(written, fixture("shielded-payment").unwrap());
    assert!(matches!(
        write_fixture("not-a-fixture", "unused.json"),
        Err(FixtureWriteError::UnknownFixture(name)) if name == "not-a-fixture"
    ));
}

#[test]
fn shielded_payment_has_no_findings() {
    assert!(rule_ids(&fixture("shielded-payment").unwrap()).is_empty());
}

#[test]
fn transparent_vectors_exercise_the_named_exposure() {
    let cases = [
        ("transparent-recipient", "SS001_TRANSPARENT_RECIPIENT"),
        ("transparent-funding", "SS002_TRANSPARENT_FUNDING"),
        ("transparent-change", "SS003_TRANSPARENT_CHANGE"),
        (
            "unified-selected-transparent",
            "SS001_TRANSPARENT_RECIPIENT",
        ),
    ];

    for (name, expected_rule) in cases {
        assert!(
            rule_ids(&fixture(name).unwrap()).contains(expected_rule),
            "{name}"
        );
    }
}

#[test]
fn unified_selected_transparent_records_the_selected_receiver_pool() {
    let snapshot = fixture("unified-selected-transparent").unwrap();
    let recipient = snapshot.steps[0]
        .outputs
        .iter()
        .find(|output| output.role == OutputRole::Recipient)
        .expect("recipient output");

    assert_eq!(recipient.pool, Pool::Transparent);
}

#[test]
fn multi_step_reports_cross_pool_balance_first_and_transparent_change_later() {
    let snapshot = fixture("multi-step-late-leak").unwrap();
    let assessment = assess(&snapshot, &Policy::standard());

    assert!(snapshot.steps.len() > 1);
    assert!(assessment.findings.iter().any(|finding| finding.rule_id
        == "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"
        && finding.step_index == 0));
    assert!(assessment
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS003_TRANSPARENT_CHANGE" && finding.step_index == 1));
}

#[test]
fn migrations_distinguish_shielded_pool_balances_from_transparent_funding() {
    // This legacy-named fixture uses only shielded addresses, but
    // Sapling→Orchard still exposes public pool-specific value balances.
    let shielded_migration = fixture("privacy-preserving-migration").unwrap();
    let shielded_rules = rule_ids(&shielded_migration);
    assert!(shielded_rules.contains("SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"));
    assert!(!shielded_rules.contains("SS002_TRANSPARENT_FUNDING"));
    assert!(!shielded_rules.contains("SS005_PRIVACY_BOUNDARY"));
    assert!(shielded_migration.steps[0]
        .outputs
        .iter()
        .all(|output| output.pool != Pool::Transparent));

    let full_balance = fixture("full-balance-migration").unwrap();
    let rules = rule_ids(&full_balance);
    assert!(rules.contains("SS002_TRANSPARENT_FUNDING"));
    assert!(rules.contains("SS005_PRIVACY_BOUNDARY"));
    assert!(!rules.contains("SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE"));
}
