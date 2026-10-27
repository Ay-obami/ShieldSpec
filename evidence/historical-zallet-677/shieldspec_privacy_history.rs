//! Test-only, independently pinned historical privacy-policy witness.
//! Identical test bytes are inserted into zallet commits immediately before and
//! after zcash/zallet#677 commit e966780. Production code is never modified.

use super::*;
use incrementalmerkletree::Position;
use nonempty::NonEmpty;
use std::collections::BTreeMap;
use zcash_client_backend::{
    data_api::wallet::{ConfirmationsPolicy, TargetHeight},
    fees::{ChangeValue, TransactionBalance},
    proposal::ShieldedInputs,
    wallet::{Note, ReceivedNote},
};
use zcash_protocol::{consensus::BlockHeight, ShieldedPool};

fn one_orchard_family_note(input_pool: ShieldedPool) -> Note {
    assert!(matches!(
        input_pool,
        ShieldedPool::Orchard | ShieldedPool::Ironwood
    ));
    let sk: orchard::keys::SpendingKey =
        Option::from(orchard::keys::SpendingKey::from_bytes([0x2a; 32]))
            .expect("valid fixed test spending key");
    let recipient = orchard::keys::FullViewingKey::from(&sk)
        .address_at(0u32, zip32::Scope::External);
    let rho =
        Option::from(orchard::note::Rho::from_bytes(&[0; 32])).expect("valid fixed rho");
    let rseed =
        Option::from(orchard::note::RandomSeed::from_bytes([0x1b; 32], &rho))
            .expect("valid fixed rseed");
    let (version, value_pool) = if input_pool == ShieldedPool::Ironwood {
        (orchard::note::NoteVersion::V3, orchard::ValuePool::Ironwood)
    } else {
        (orchard::note::NoteVersion::V2, orchard::ValuePool::Orchard)
    };
    Note::Orchard {
        note: Option::from(orchard::note::Note::from_parts(
            recipient,
            orchard::value::NoteValue::from_raw(20_000),
            rho,
            rseed,
            version,
        ))
        .expect("valid fixed note"),
        pool: value_pool,
    }
}

fn actual_wallet_proposal(input_pool: ShieldedPool, output_pool: ShieldedPool) -> Proposal<(), u32> {
    let received = ReceivedNote::from_parts(
        0u32,
        TxId::from_bytes([0; 32]),
        0u16,
        one_orchard_family_note(input_pool),
        zip32::Scope::External,
        Position::from(0u64),
        Some(BlockHeight::from_u32(100)),
        None,
    );
    let inputs =
        ShieldedInputs::from_parts(NonEmpty::from_vec(vec![received]).expect("one input note"));
    let change = ChangeValue::shielded(
        output_pool,
        Zatoshis::const_from_u64(10_000),
        None,
    );
    Proposal::single_step(
        TransactionRequest::empty(),
        BTreeMap::new(),
        vec![],
        Some(inputs),
        BlockHeight::from_u32(100),
        TransactionBalance::new(vec![change], Zatoshis::const_from_u64(10_000))
            .expect("balanced test proposal"),
        (),
        TargetHeight::from(BlockHeight::from_u32(101)),
        ConfirmationsPolicy::default(),
        false,
        false,
    )
    .expect("valid real proposal")
}

/// Security property: an Ironwood input funding Sapling change reveals the
/// crossing amount through public value balances despite both pools being
/// shielded. FullPrivacy must not silently authorize that crossing.
#[test]
fn shieldspec_ironwood_to_sapling_full_privacy_rejected() {
    let proposal = actual_wallet_proposal(ShieldedPool::Ironwood, ShieldedPool::Sapling);
    let result = enforce_privacy_policy(&proposal, PrivacyPolicy::FullPrivacy);
    println!("SHIELDSPEC_IRONWOOD_TO_SAPLING_FULL_PRIVACY accepted={}", result.is_ok());
    assert!(
        result.is_err(),
        "Ironwood -> Sapling crosses a publicly visible value balance; FullPrivacy must reject"
    );
}

/// Control: the older implementation already rejected Orchard -> Sapling.
#[test]
fn shieldspec_orchard_to_sapling_control_rejected() {
    let proposal = actual_wallet_proposal(ShieldedPool::Orchard, ShieldedPool::Sapling);
    let result = enforce_privacy_policy(&proposal, PrivacyPolicy::FullPrivacy);
    println!("SHIELDSPEC_ORCHARD_TO_SAPLING_FULL_PRIVACY accepted={}", result.is_ok());
    assert!(result.is_err(), "existing Orchard -> Sapling guard must still work");
}

/// Control: returning change to the same shielded pool reveals no crossing.
#[test]
fn shieldspec_ironwood_same_pool_control_allowed() {
    let proposal = actual_wallet_proposal(ShieldedPool::Ironwood, ShieldedPool::Ironwood);
    let result = enforce_privacy_policy(&proposal, PrivacyPolicy::FullPrivacy);
    println!("SHIELDSPEC_IRONWOOD_TO_IRONWOOD_FULL_PRIVACY accepted={}", result.is_ok());
    assert!(result.is_ok(), "same-pool change must remain permissible");
}
