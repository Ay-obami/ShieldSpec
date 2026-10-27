use shieldspec_core::{assess, InputOrigin, OutputRole, Policy, Pool};
use shieldspec_zcash_runner::{
    run_transparent_funded_payment, ShieldedPoolChoice, TransparentFundedPaymentScenario,
};

#[test]
fn real_transparent_utxo_funding_shielded_recipient_is_reported() {
    let scenario = TransparentFundedPaymentScenario {
        funding_zat: 150_000,
        payment_zat: 75_000,
        utxo_count: 2,
        confirmations: 2,
        recipient_pool: ShieldedPoolChoice::Sapling,
        change_pool: ShieldedPoolChoice::Orchard,
    };
    let snapshot =
        run_transparent_funded_payment(&scenario).expect("real transparent-funded proposal");
    assert!(snapshot
        .steps
        .iter()
        .flat_map(|s| &s.inputs)
        .any(|i| i.pool == Pool::Transparent && i.origin == InputOrigin::Wallet));
    assert!(snapshot
        .steps
        .iter()
        .flat_map(|s| &s.outputs)
        .any(|o| o.pool == Pool::Sapling
            && o.role == OutputRole::Recipient
            && o.value_zat == scenario.payment_zat));
    let assessment = assess(&snapshot, &Policy::standard());
    assert!(assessment
        .findings
        .iter()
        .any(|f| f.rule_id == "SS002_TRANSPARENT_FUNDING"));
}
