use std::collections::BTreeMap;

use nonempty::NonEmpty;
use shieldspec_core::{
    InputFact, InputOrigin, OutputFact, OutputRole, Pool, PriorStepOutput, StepSnapshot,
};
use shieldspec_zcash::snapshot_from_proposal;
use zcash_client_backend::{
    data_api::wallet::{ConfirmationsPolicy, TargetHeight},
    fees::{ChangeValue, TransactionBalance},
    proposal::{Proposal, Step, StepOutput, StepOutputIndex},
    wallet::WalletTransparentOutput,
};
use zcash_keys::address::Address;
use zcash_protocol::{
    consensus::{BlockHeight, Network},
    value::Zatoshis,
    PoolType,
};
use zcash_transparent::{
    address::TransparentAddress,
    bundle::{OutPoint, TxOut},
};
use zip321::{Payment, TransactionRequest};

const INPUT_VALUE: u64 = 100_000;
const PAYMENT_VALUE: u64 = 60_000;
const CHANGE_VALUE: u64 = 30_000;
const FEE_VALUE: u64 = 10_000;

fn transparent_request(amount: u64, tag: u8) -> TransactionRequest {
    let recipient = Address::Transparent(TransparentAddress::PublicKeyHash([tag; 20]));
    TransactionRequest::new(vec![Payment::new(
        recipient.to_zcash_address(&Network::TestNetwork),
        Some(Zatoshis::const_from_u64(amount)),
        None,
        None,
        None,
        vec![],
    )
    .expect("valid payment")])
    .expect("valid request")
}

fn transparent_input(value: u64) -> WalletTransparentOutput<()> {
    let address = TransparentAddress::PublicKeyHash([0x22; 20]);
    WalletTransparentOutput::from_parts(
        OutPoint::new([0x33; 32], 0),
        TxOut::new(Zatoshis::const_from_u64(value), address.script().into()),
        Some(BlockHeight::from_u32(90)),
        None,
        None,
        None,
    )
    .expect("synthetic transparent input")
}

fn transparent_pools() -> BTreeMap<usize, PoolType> {
    BTreeMap::from([(0, PoolType::TRANSPARENT)])
}

fn transparent_balance(change: u64, fee: u64) -> TransactionBalance {
    TransactionBalance::new(
        vec![ChangeValue::transparent(Zatoshis::const_from_u64(change))],
        Zatoshis::const_from_u64(fee),
    )
    .expect("valid balance")
}

fn ephemeral_transparent_balance(value: u64, fee: u64) -> TransactionBalance {
    TransactionBalance::new(
        vec![ChangeValue::ephemeral_transparent(
            Zatoshis::const_from_u64(value),
        )],
        Zatoshis::const_from_u64(fee),
    )
    .expect("valid ephemeral balance")
}

fn transparent_proposal() -> Proposal<(), u32> {
    Proposal::single_step(
        transparent_request(PAYMENT_VALUE, 0x11),
        transparent_pools(),
        vec![transparent_input(INPUT_VALUE)],
        None,
        BlockHeight::from_u32(100),
        transparent_balance(CHANGE_VALUE, FEE_VALUE),
        (),
        TargetHeight::from(BlockHeight::from_u32(101)),
        ConfirmationsPolicy::default(),
        false,
        false,
    )
    .expect("valid SDK proposal")
}

#[test]
fn snapshots_transparent_funding_selected_payment_change_and_fee() {
    let snapshot = snapshot_from_proposal(&transparent_proposal(), "testnet")
        .expect("adapter preserves SDK proposal facts");

    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.network, "testnet");
    assert_eq!(
        snapshot.steps,
        vec![StepSnapshot {
            index: 0,
            inputs: vec![InputFact {
                pool: Pool::Transparent,
                value_zat: INPUT_VALUE,
                origin: Default::default(),
            }],
            outputs: vec![
                OutputFact {
                    pool: Pool::Transparent,
                    role: OutputRole::Recipient,
                    value_zat: PAYMENT_VALUE,
                },
                OutputFact {
                    pool: Pool::Transparent,
                    role: OutputRole::Change,
                    value_zat: CHANGE_VALUE,
                },
            ],
            fee_zat: FEE_VALUE,
        }]
    );
}

#[test]
fn snapshots_ephemeral_output_and_later_input_with_transparent_pool_and_value() {
    let first: Step<u32> = Step::from_parts(
        &[],
        transparent_request(PAYMENT_VALUE, 0x41),
        transparent_pools(),
        vec![transparent_input(INPUT_VALUE)],
        None,
        None,
        vec![],
        ephemeral_transparent_balance(CHANGE_VALUE, FEE_VALUE),
        false,
        false,
    )
    .expect("first step");
    let second = Step::from_parts(
        std::slice::from_ref(&first),
        transparent_request(20_000, 0x42),
        transparent_pools(),
        vec![],
        None,
        None,
        vec![StepOutput::new(0, StepOutputIndex::Change(0))],
        TransactionBalance::new(vec![], Zatoshis::const_from_u64(FEE_VALUE))
            .expect("second balance"),
        false,
        false,
    )
    .expect("second step");
    let proposal = Proposal::multi_step(
        (),
        TargetHeight::from(BlockHeight::from_u32(101)),
        ConfirmationsPolicy::default(),
        NonEmpty::from_vec(vec![first, second]).expect("nonempty steps"),
    )
    .expect("valid SDK multi-step proposal");

    let snapshot = snapshot_from_proposal(&proposal, "testnet").expect("snapshot");

    assert_eq!(snapshot.steps.len(), 2);
    assert_eq!(
        snapshot.steps[0],
        StepSnapshot {
            index: 0,
            inputs: vec![InputFact {
                pool: Pool::Transparent,
                value_zat: INPUT_VALUE,
                origin: Default::default(),
            }],
            outputs: vec![
                OutputFact {
                    pool: Pool::Transparent,
                    role: OutputRole::Recipient,
                    value_zat: PAYMENT_VALUE,
                },
                OutputFact {
                    pool: Pool::Transparent,
                    role: OutputRole::Ephemeral,
                    value_zat: CHANGE_VALUE,
                },
            ],
            fee_zat: FEE_VALUE,
        }
    );
    assert_eq!(snapshot.steps[1].index, 1);
    assert_eq!(
        snapshot.steps[1].inputs,
        vec![InputFact {
            pool: Pool::Transparent,
            value_zat: CHANGE_VALUE,
            origin: InputOrigin::PriorStep {
                step_index: 0,
                output: PriorStepOutput::Change(0),
            },
        }]
    );
    assert_eq!(
        snapshot.steps[1].outputs,
        vec![OutputFact {
            pool: Pool::Transparent,
            role: OutputRole::Recipient,
            value_zat: 20_000,
        }]
    );
    assert_eq!(snapshot.steps[1].fee_zat, FEE_VALUE);
}
