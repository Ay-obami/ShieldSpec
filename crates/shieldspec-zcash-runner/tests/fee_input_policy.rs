use shieldspec_core::{InputOrigin, OutputRole, Pool};
use shieldspec_zcash_runner::{
    run_orchard_payment, run_tex_payment, OrchardPaymentScenario, ShieldedPoolChoice,
    TexPaymentScenario,
};

fn wallet_input_count(snapshot: &shieldspec_core::ProposalSnapshot) -> usize {
    snapshot
        .steps
        .iter()
        .flat_map(|step| step.inputs.iter())
        .filter(|input| input.origin == InputOrigin::Wallet)
        .count()
}

fn assert_step_balances(snapshot: &shieldspec_core::ProposalSnapshot) {
    for step in &snapshot.steps {
        let input_total: u64 = step.inputs.iter().map(|input| input.value_zat).sum();
        let output_total: u64 = step.outputs.iter().map(|output| output.value_zat).sum();
        assert_eq!(
            input_total,
            output_total + step.fee_zat,
            "SDK proposal step {} must conserve value",
            step.index
        );
    }
}

#[test]
fn fragmented_orchard_funding_changes_real_wallet_input_selection() {
    let mut scenario = OrchardPaymentScenario::demo();
    scenario.funding_zat = 300_000;
    scenario.payment_zat = 210_000;
    scenario.note_count = 1;
    let consolidated = run_orchard_payment(&scenario).expect("consolidated proposal");
    scenario.note_count = 4;
    let fragmented = run_orchard_payment(&scenario).expect("fragmented proposal");

    assert_eq!(wallet_input_count(&consolidated), 1);
    assert!(
        wallet_input_count(&fragmented) >= 3,
        "payment requires at least three 75k notes"
    );
    assert!(wallet_input_count(&fragmented) > wallet_input_count(&consolidated));
    assert_step_balances(&consolidated);
    assert_step_balances(&fragmented);
}

#[test]
fn fragmented_sapling_funding_changes_real_wallet_input_selection() {
    let mut scenario = OrchardPaymentScenario::demo();
    scenario.funding_zat = 300_000;
    scenario.payment_zat = 210_000;
    scenario.funding_pool = ShieldedPoolChoice::Sapling;
    scenario.change_pool = ShieldedPoolChoice::Sapling;
    scenario.note_count = 1;
    let consolidated = run_orchard_payment(&scenario).expect("consolidated sapling proposal");
    scenario.note_count = 4;
    let fragmented = run_orchard_payment(&scenario).expect("fragmented sapling proposal");

    assert_eq!(wallet_input_count(&consolidated), 1);
    assert!(wallet_input_count(&fragmented) >= 3);
    assert!(fragmented.steps[0]
        .inputs
        .iter()
        .all(|input| input.pool == Pool::Sapling));
    assert_step_balances(&consolidated);
    assert_step_balances(&fragmented);
}

#[test]
fn fee_and_change_are_observed_from_real_zip317_wallet_proposals() {
    for funding_pool in [ShieldedPoolChoice::Orchard, ShieldedPoolChoice::Sapling] {
        for note_count in [1, 2, 4] {
            let mut scenario = OrchardPaymentScenario::demo();
            scenario.funding_zat = 400_000;
            scenario.payment_zat = 250_000;
            scenario.funding_pool = funding_pool;
            scenario.change_pool = funding_pool;
            scenario.note_count = note_count;
            let snapshot = run_orchard_payment(&scenario).expect("real ZIP317 proposal");
            assert_eq!(snapshot.steps.len(), 1);
            assert!(snapshot.steps[0].fee_zat > 0);
            assert!(snapshot.steps[0]
                .outputs
                .iter()
                .any(|output| output.role == OutputRole::Change));
            assert_step_balances(&snapshot);
            assert_eq!(
                snapshot,
                run_orchard_payment(&scenario).expect("deterministic replay")
            );
        }
    }
}

#[test]
fn two_step_tex_fee_accounting_never_counts_ephemeral_inputs_as_wallet_funding() {
    let mut scenario = TexPaymentScenario::demo();
    scenario.funding_zat = 400_000;
    scenario.payment_zat = 250_000;
    scenario.note_count = 4;
    let snapshot = run_tex_payment(&scenario).expect("real ZIP320 proposal");
    assert_eq!(snapshot.steps.len(), 2);
    assert!(snapshot.steps[0]
        .inputs
        .iter()
        .any(|input| input.origin == InputOrigin::Wallet));
    assert!(snapshot.steps[1]
        .inputs
        .iter()
        .all(|input| input.origin != InputOrigin::Wallet));
    assert_step_balances(&snapshot);
}
