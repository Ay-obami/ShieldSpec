use shieldspec_core::{assess, InputOrigin, OutputRole, Policy, Pool, PriorStepOutput};
use shieldspec_zcash_runner::{
    run_orchard_payment, run_tex_payment, run_unified_payment, OrchardPaymentScenario,
    ShieldedPoolChoice, TexPaymentScenario, UnifiedPaymentScenario, UnifiedReceiverSet,
};

#[test]
fn orchard_scenario_runs_through_real_wallet_proposal_machinery() {
    let scenario = OrchardPaymentScenario::demo();
    let snapshot = run_orchard_payment(&scenario).expect("real proposal");

    assert_eq!(snapshot.network, "local-test");
    assert_eq!(snapshot.steps.len(), 1);

    let step = &snapshot.steps[0];
    assert!(!step.inputs.is_empty());
    assert!(step
        .inputs
        .iter()
        .all(|input| input.pool == Pool::Orchard && input.origin == InputOrigin::Wallet));
    assert!(step.outputs.iter().any(|output| {
        output.pool == Pool::Orchard
            && output.role == OutputRole::Recipient
            && output.value_zat == scenario.payment_zat
    }));
    assert!(step
        .outputs
        .iter()
        .any(|output| { output.pool == Pool::Orchard && output.role == OutputRole::Change }));
    assert!(step.fee_zat > 0);

    let assessment = assess(&snapshot, &Policy::standard());
    assert!(
        assessment.passes,
        "shielded Orchard proposal should pass: {:?}",
        assessment.findings
    );
}

#[test]
fn tex_scenario_uses_real_ephemeral_two_step_proposal() {
    let scenario = TexPaymentScenario::demo();
    let snapshot = run_tex_payment(&scenario).expect("real ZIP 320 proposal");

    assert_eq!(snapshot.steps.len(), 2);
    let first = &snapshot.steps[0];
    let second = &snapshot.steps[1];

    let ephemeral_index = first
        .outputs
        .iter()
        .position(|output| output.pool == Pool::Transparent && output.role == OutputRole::Ephemeral)
        .expect("first step has ephemeral transparent output");

    assert!(second.inputs.iter().any(|input| {
        input.pool == Pool::Transparent
            && matches!(
                input.origin,
                InputOrigin::PriorStep {
                    step_index: 0,
                    output: PriorStepOutput::Change(index),
                } if index == ephemeral_index
            )
    }));
    assert!(second.outputs.iter().any(|output| {
        output.pool == Pool::Transparent
            && output.role == OutputRole::Recipient
            && output.value_zat == scenario.payment_zat
    }));

    let assessment = assess(&snapshot, &Policy::standard());
    assert!(assessment
        .findings
        .iter()
        .all(|finding| finding.rule_id != "SS002_TRANSPARENT_FUNDING"));
    assert!(assessment
        .findings
        .iter()
        .all(|finding| finding.rule_id != "SS003_TRANSPARENT_CHANGE"));
    assert!(assessment
        .findings
        .iter()
        .any(|finding| finding.rule_id == "SS001_TRANSPARENT_RECIPIENT"));
}

#[test]
fn identical_scenario_replays_to_identical_snapshot() {
    let scenario = OrchardPaymentScenario::demo();

    let first = run_orchard_payment(&scenario).expect("first proposal");
    let second = run_orchard_payment(&scenario).expect("second proposal");

    assert_eq!(first, second);
}

#[test]
fn identical_tex_scenario_replays_to_identical_snapshot() {
    let scenario = TexPaymentScenario::demo();

    let first = run_tex_payment(&scenario).expect("first proposal");
    let second = run_tex_payment(&scenario).expect("second proposal");

    assert_eq!(first, second);
}

#[test]
fn invalid_scenario_is_rejected_before_wallet_execution() {
    let mut scenario = OrchardPaymentScenario::demo();
    scenario.payment_zat = scenario.funding_zat;

    let error = run_orchard_payment(&scenario).expect_err("invalid scenario");
    assert!(error.to_string().contains("leave room"));
}

#[test]
fn mixed_unified_addresses_keep_recipient_delivery_shielded() {
    for receivers in [
        UnifiedReceiverSet::OrchardSapling,
        UnifiedReceiverSet::OrchardTransparent,
        UnifiedReceiverSet::SaplingTransparent,
        UnifiedReceiverSet::OrchardSaplingTransparent,
    ] {
        for funding_pool in [ShieldedPoolChoice::Orchard, ShieldedPoolChoice::Sapling] {
            let scenario = UnifiedPaymentScenario {
                funding_zat: 120_000,
                payment_zat: 75_000,
                note_count: 2,
                confirmations: 2,
                empty_blocks: 1,
                funding_pool,
                change_pool: ShieldedPoolChoice::Orchard,
                receivers,
            };

            let snapshot = run_unified_payment(&scenario).expect("real Unified Address proposal");
            let recipient_outputs = snapshot
                .steps
                .iter()
                .flat_map(|step| step.outputs.iter())
                .filter(|output| output.role == OutputRole::Recipient)
                .collect::<Vec<_>>();

            assert_eq!(recipient_outputs.len(), 1);
            assert!(
                matches!(
                    recipient_outputs[0].pool,
                    Pool::Sapling | Pool::Orchard | Pool::Ironwood
                ),
                "supported shielded receiver should win over transparent: {scenario:?} -> {snapshot:?}"
            );

            let assessment = assess(&snapshot, &Policy::standard());
            assert!(
                assessment.passes,
                "mixed Unified Address should remain shielded: {scenario:?} -> {:?}",
                assessment.findings
            );
        }
    }
}
