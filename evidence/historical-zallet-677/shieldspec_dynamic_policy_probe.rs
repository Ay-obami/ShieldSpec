//! Dynamic test-only ShieldSpec adapter injected into both independently pinned
//! Zallet revisions. No production policy code or old fixed witness is changed.
//! One environment-supplied case builds a genuine Zallet Proposal and executes
//! the private enforce_privacy_policy implementation in this source module.

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
use zcash_protocol::{consensus::BlockHeight, PoolType, ShieldedPool};

const PROBE_MARKER: &str = "SHIELDSPEC_POLICY_PROBE_JSON=";

fn probe_pool_name(pool: ShieldedPool) -> &'static str {
    match pool {
        ShieldedPool::Sapling => "sapling",
        ShieldedPool::Orchard => "orchard",
        ShieldedPool::Ironwood => "ironwood",
    }
}

fn probe_parse_pool(raw: &str) -> ShieldedPool {
    match raw {
        "sapling" => ShieldedPool::Sapling,
        "orchard" => ShieldedPool::Orchard,
        "ironwood" => ShieldedPool::Ironwood,
        _ => panic!("unsupported shielded pool"),
    }
}

fn probe_orchard_note(input_pool: ShieldedPool, input_zat: u64) -> Note {
    assert!(matches!(
        input_pool,
        ShieldedPool::Orchard | ShieldedPool::Ironwood
    ));
    let sk: orchard::keys::SpendingKey =
        Option::from(orchard::keys::SpendingKey::from_bytes([0x2a; 32]))
            .expect("valid fixed test spending key");
    let recipient =
        orchard::keys::FullViewingKey::from(&sk).address_at(0u32, zip32::Scope::External);
    let rho = Option::from(orchard::note::Rho::from_bytes(&[0; 32])).expect("valid fixed rho");
    let rseed = Option::from(orchard::note::RandomSeed::from_bytes([0x1b; 32], &rho))
        .expect("valid fixed rseed");
    let (version, value_pool) = if input_pool == ShieldedPool::Ironwood {
        (orchard::note::NoteVersion::V3, orchard::ValuePool::Ironwood)
    } else {
        (orchard::note::NoteVersion::V2, orchard::ValuePool::Orchard)
    };
    Note::Orchard {
        note: Option::from(orchard::note::Note::from_parts(
            recipient,
            orchard::value::NoteValue::from_raw(input_zat),
            rho,
            rseed,
            version,
        ))
        .expect("valid real test note"),
        pool: value_pool,
    }
}

fn probe_proposal(
    input_pool: ShieldedPool,
    change_pool: ShieldedPool,
    input_zat: u64,
    change_zat: u64,
    fee_zat: u64,
) -> Proposal<(), u32> {
    assert!(input_zat > fee_zat && fee_zat > 0);
    assert_eq!(input_zat.checked_sub(fee_zat), Some(change_zat));
    let received = ReceivedNote::from_parts(
        0u32,
        TxId::from_bytes([0; 32]),
        0u16,
        probe_orchard_note(input_pool, input_zat),
        zip32::Scope::External,
        Position::from(0u64),
        Some(BlockHeight::from_u32(100)),
        None,
    );
    let inputs = ShieldedInputs::from_parts(NonEmpty::from_vec(vec![received]).expect("one input"));
    let change = ChangeValue::shielded(change_pool, Zatoshis::const_from_u64(change_zat), None);
    Proposal::single_step(
        TransactionRequest::empty(),
        BTreeMap::new(),
        vec![],
        Some(inputs),
        BlockHeight::from_u32(100),
        TransactionBalance::new(vec![change], Zatoshis::const_from_u64(fee_zat))
            .expect("balanced test proposal"),
        (),
        TargetHeight::from(BlockHeight::from_u32(101)),
        ConfirmationsPolicy::default(),
        false,
        false,
    )
    .expect("valid actual wallet Proposal")
}

/// Unlike the original fixed historical witness, no verdict is asserted or
/// supplied in the scenario. Both revisions return their actual policy decision.
#[test]
fn shieldspec_policy_probe_dynamic() {
    let raw = std::env::var("SHIELDSPEC_ZALLET_SCENARIO_JSON")
        .expect("canonical scenario must be provided by ShieldSpec");
    let input: serde_json::Value =
        serde_json::from_str(&raw).expect("valid JSON scenario parameters");
    let field = |name: &str| input.get(name).expect("required probe parameter");
    let input_pool = probe_parse_pool(field("input_pool").as_str().expect("input pool name"));
    let change_pool = probe_parse_pool(field("change_pool").as_str().expect("change pool name"));
    assert!(matches!(
        input_pool,
        ShieldedPool::Orchard | ShieldedPool::Ironwood
    ));
    let input_zat = field("input_zat").as_u64().expect("input amount");
    let change_zat = field("change_zat").as_u64().expect("change amount");
    let fee_zat = field("fee_zat").as_u64().expect("fee amount");
    let proposal = probe_proposal(input_pool, change_pool, input_zat, change_zat, fee_zat);

    // Read back what the compiled, typed Proposal actually contains; do not
    // echo requested values as if they were observed.
    let step = &proposal.steps().head;
    let actual_inputs = step.shielded_inputs().expect("real shielded inputs");
    let received_notes = actual_inputs.notes();
    let actual_input = received_notes.iter().next().expect("one real input note");
    let actual_changes = step.balance().proposed_change();
    let actual_change = actual_changes.first().expect("one real change output");
    let observed_change_pool = match actual_change.output_pool() {
        PoolType::Shielded(pool) => pool,
        PoolType::Transparent => panic!("unexpected transparent change"),
    };
    let observed = serde_json::json!({
        "step_count": proposal.steps().iter().count(),
        "input_count": received_notes.iter().count(),
        "change_count": actual_changes.len(),
        "recipient_count": step.payment_pools().len(),
        "input_pool": probe_pool_name(actual_input.note().pool()),
        "change_pool": probe_pool_name(observed_change_pool),
        "input_zat": actual_input.note().value().into_u64(),
        "change_zat": actual_change.value().into_u64(),
        "fee_zat": step.balance().fee_required().into_u64(),
    });
    let accepted = enforce_privacy_policy(&proposal, PrivacyPolicy::FullPrivacy).is_ok();
    let record = serde_json::json!({
        "schema_version": 1,
        "observed_proposal": observed,
        "policy": "zallet_full_privacy_v1",
        "accepted": accepted,
    });
    println!("{PROBE_MARKER}{record}");
}
