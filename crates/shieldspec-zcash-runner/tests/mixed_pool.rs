use shieldspec_core::{InputOrigin, Pool};
use shieldspec_zcash_runner::{
    run_mixed_pool_payment, MixedPoolPaymentScenario, ShieldedPoolChoice,
};

#[test]
fn mixed_sapling_and_orchard_funds_are_selected_from_one_real_wallet() {
    let scenario = MixedPoolPaymentScenario {
        sapling_funding_zat: 60_000,
        orchard_funding_zat: 20_000,
        payment_zat: 52_000,
        recipient_pool: ShieldedPoolChoice::Sapling,
        change_pool: ShieldedPoolChoice::Orchard,
        confirmations: 1,
    };
    let snapshot = run_mixed_pool_payment(&scenario).expect("real mixed-pool proposal");
    let inputs = &snapshot.steps[0].inputs;
    assert!(inputs
        .iter()
        .any(|input| input.pool == Pool::Sapling && input.origin == InputOrigin::Wallet));
    assert!(inputs
        .iter()
        .any(|input| input.pool == Pool::Orchard && input.origin == InputOrigin::Wallet));
    assert_eq!(
        snapshot,
        run_mixed_pool_payment(&scenario).expect("deterministic replay")
    );
}

#[test]
fn pinned_backend_reproduces_documented_mixed_pool_dust_shortfall() {
    // This is librustzcash #3133's 60k Sapling + 20k Orchard / 47k payment
    // case. The pinned 0.25.0-pre.2 selector rejects this otherwise-funded
    // proposal; an SDK upgrade that fixes it should prompt revisiting this
    // expected historical outcome, not silently passing this regression test.
    let scenario = MixedPoolPaymentScenario {
        sapling_funding_zat: 60_000,
        orchard_funding_zat: 20_000,
        payment_zat: 47_000,
        recipient_pool: ShieldedPoolChoice::Sapling,
        change_pool: ShieldedPoolChoice::Orchard,
        confirmations: 1,
    };
    let error = run_mixed_pool_payment(&scenario).expect_err("pinned SDK historical shortfall");
    let message = error.to_string();
    assert!(message.contains("InsufficientFunds"), "{message}");
    assert!(message.contains("80000"), "{message}");
    assert!(message.contains("62000"), "{message}");
}
