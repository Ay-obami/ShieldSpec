//! Deterministic real-wallet proposal runner for ShieldSpec fuzzing.
//!
//! This crate deliberately uses librustzcash's downstream test harness to create
//! realistic wallet state and invoke the real proposal machinery. It is not
//! published as a production dependency.

use std::{convert::Infallible, error::Error, fmt};

use serde::{Deserialize, Serialize};
use shieldspec_core::ProposalSnapshot;
use shieldspec_fuzz::{
    DifferentialProvenance, WalletPolicyDecision, WalletPolicyDifference, WalletPolicyVerdict,
};
use shieldspec_zcash::{snapshot_from_proposal, AdapterError};
use zcash_client_backend::{
    data_api::{
        error::Error as WalletError,
        testing::single_output_change_strategy,
        testing::{
            orchard::OrchardPoolTester,
            pool::{dsl::TestDsl, ShieldedPoolTester},
            sapling::SaplingPoolTester,
            AddressType, TestBuilder,
        },
        wallet::input_selection::{GreedyInputSelector, SpendPolicy, TransparentSpendPolicy},
        wallet::ConfirmationsPolicy,
        Account, WalletRead, WalletWrite,
    },
    fees::{standard::SingleOutputChangeStrategy, DustOutputPolicy, StandardFeeRule},
    wallet::WalletTransparentOutput,
    zip321::{Payment, TransactionRequest},
};
use zcash_client_sqlite::testing::{db::TestDbFactory, BlockCache};
use zcash_keys::{
    address::{Address, UnifiedAddress},
    keys::{transparent::gap_limits::GapLimits, UnifiedAddressRequest},
};
use zcash_primitives::block::BlockHash;
use zcash_protocol::{
    consensus::{BlockHeight, NetworkUpgrade, Parameters},
    local_consensus::LocalNetwork,
    value::Zatoshis,
    ShieldedPool,
};
use zcash_transparent::{
    bundle::{OutPoint, TxOut},
    keys::TransparentKeyScope,
};

/// Shielded funding or change pool used by a fuzzing scenario.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShieldedPoolChoice {
    Sapling,
    Orchard,
}

impl ShieldedPoolChoice {
    const fn protocol(self) -> ShieldedPool {
        match self {
            Self::Sapling => ShieldedPool::Sapling,
            Self::Orchard => ShieldedPool::Orchard,
        }
    }

    const fn complexity_weight(self) -> u128 {
        match self {
            Self::Sapling => 1,
            Self::Orchard => 0,
        }
    }

    const fn alternate(self) -> Self {
        match self {
            Self::Sapling => Self::Orchard,
            Self::Orchard => Self::Sapling,
        }
    }
}

/// Small deterministic scenario used to prove the real proposal harness.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchardPaymentScenario {
    pub funding_zat: u64,
    pub payment_zat: u64,
    pub recipient_seed: u8,
    pub note_count: u8,
    pub confirmations: u8,
    pub empty_blocks: u8,
    pub funding_pool: ShieldedPoolChoice,
    pub change_pool: ShieldedPoolChoice,
    /// Shielded receiver selected for the payment. Omitted in older corpus
    /// records, which always targeted Orchard.
    #[serde(
        default = "default_orchard_pool",
        skip_serializing_if = "is_orchard_pool"
    )]
    pub recipient_pool: ShieldedPoolChoice,
}

const fn default_orchard_pool() -> ShieldedPoolChoice {
    ShieldedPoolChoice::Orchard
}

fn is_orchard_pool(pool: &ShieldedPoolChoice) -> bool {
    *pool == ShieldedPoolChoice::Orchard
}

impl OrchardPaymentScenario {
    #[must_use]
    pub const fn demo() -> Self {
        Self {
            funding_zat: 120_000,
            payment_zat: 100_000,
            recipient_seed: 0x42,
            note_count: 1,
            confirmations: 1,
            empty_blocks: 0,
            funding_pool: ShieldedPoolChoice::Orchard,
            change_pool: ShieldedPoolChoice::Orchard,
            recipient_pool: ShieldedPoolChoice::Orchard,
        }
    }
}

/// The proposal epoch relative to NU6.3 activation on a synthetic local network.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Nu63Activation {
    Before,
    At,
}

/// An opt-in, activation-aware Orchard-receiver proposal.
///
/// Ironwood shares Orchard receiver addresses; the real SDK must select the
/// output pool. This scenario does not pretend to fund an Ironwood note.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Nu63OrchardPaymentScenario {
    pub payment: OrchardPaymentScenario,
    pub activation: Nu63Activation,
}

/// Deterministic ZIP 320 scenario that pays a TEX recipient from Orchard funds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TexPaymentScenario {
    pub funding_zat: u64,
    pub payment_zat: u64,
    pub recipient_tag: u8,
    pub note_count: u8,
    pub confirmations: u8,
    pub empty_blocks: u8,
    pub funding_pool: ShieldedPoolChoice,
    pub change_pool: ShieldedPoolChoice,
}

impl TexPaymentScenario {
    #[must_use]
    pub const fn demo() -> Self {
        Self {
            funding_zat: 100_000,
            payment_zat: 50_000,
            recipient_tag: 0x04,
            note_count: 1,
            confirmations: 1,
            empty_blocks: 0,
            funding_pool: ShieldedPoolChoice::Orchard,
            change_pool: ShieldedPoolChoice::Orchard,
        }
    }
}

/// Receiver combinations used to exercise Unified Address privacy selection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedReceiverSet {
    OrchardSapling,
    OrchardTransparent,
    SaplingTransparent,
    OrchardSaplingTransparent,
}

impl UnifiedReceiverSet {
    const fn includes_orchard(self) -> bool {
        matches!(
            self,
            Self::OrchardSapling | Self::OrchardTransparent | Self::OrchardSaplingTransparent
        )
    }

    const fn includes_sapling(self) -> bool {
        matches!(
            self,
            Self::OrchardSapling | Self::SaplingTransparent | Self::OrchardSaplingTransparent
        )
    }

    const fn includes_transparent(self) -> bool {
        matches!(
            self,
            Self::OrchardTransparent | Self::SaplingTransparent | Self::OrchardSaplingTransparent
        )
    }

    const fn complexity_weight(self) -> u128 {
        match self {
            Self::OrchardSapling => 0,
            Self::OrchardTransparent | Self::SaplingTransparent => 1,
            Self::OrchardSaplingTransparent => 2,
        }
    }
}

/// A payment to a Unified Address with controlled receiver composition.
///
/// Mixed shielded/transparent receiver sets are privacy-sensitive: when a
/// supported shielded receiver exists, selecting the transparent receiver would
/// be a meaningful proposal-level regression for ShieldSpec to catch.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnifiedPaymentScenario {
    pub funding_zat: u64,
    pub payment_zat: u64,
    pub note_count: u8,
    pub confirmations: u8,
    pub empty_blocks: u8,
    pub funding_pool: ShieldedPoolChoice,
    pub change_pool: ShieldedPoolChoice,
    pub receivers: UnifiedReceiverSet,
}

/// A real wallet holding spendable Sapling and Orchard notes simultaneously.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MixedPoolPaymentScenario {
    pub sapling_funding_zat: u64,
    pub orchard_funding_zat: u64,
    pub payment_zat: u64,
    pub recipient_pool: ShieldedPoolChoice,
    pub change_pool: ShieldedPoolChoice,
    pub confirmations: u8,
}

/// Exercises genuine cross-pool input selection, including insufficient-funds
/// outcomes, through librustzcash's wallet proposal machinery.
pub fn run_mixed_pool_payment(
    scenario: &MixedPoolPaymentScenario,
) -> Result<ProposalSnapshot, RunnerError> {
    if scenario.sapling_funding_zat == 0 || scenario.orchard_funding_zat == 0 {
        return Err(RunnerError::InvalidScenario(
            "both funding pools must be nonempty",
        ));
    }
    let funding_total = scenario
        .sapling_funding_zat
        .checked_add(scenario.orchard_funding_zat)
        .ok_or(RunnerError::InvalidScenario("funding total overflow"))?;
    validate_amounts(
        funding_total,
        scenario.payment_zat,
        2,
        scenario.confirmations,
    )?;

    let mut wallet =
        TestDsl::with_sapling_birthday_account(TestDbFactory::default(), BlockCache::new())
            .map(|builder| builder.with_gap_limits(GapLimits::new(10, 5, 3)))
            .build::<OrchardPoolTester>();

    let sapling_fvk = SaplingPoolTester::test_account_fvk(&wallet);
    let (sapling_height, _, _) = wallet.generate_next_block(
        &sapling_fvk,
        AddressType::DefaultExternal,
        Zatoshis::const_from_u64(scenario.sapling_funding_zat),
    );
    wallet.scan_cached_blocks(sapling_height, 1);

    let orchard_fvk = OrchardPoolTester::test_account_fvk(&wallet);
    let (orchard_height, _, _) = wallet.generate_next_block(
        &orchard_fvk,
        AddressType::DefaultExternal,
        Zatoshis::const_from_u64(scenario.orchard_funding_zat),
    );
    wallet.scan_cached_blocks(orchard_height, 1);
    if scenario.confirmations > 1 {
        wallet.add_empty_blocks(usize::from(scenario.confirmations - 1));
    }

    let recipient = match scenario.recipient_pool {
        ShieldedPoolChoice::Sapling => {
            let sk = SaplingPoolTester::sk(&[0x61; 32]);
            SaplingPoolTester::sk_default_address(&sk)
        }
        ShieldedPoolChoice::Orchard => {
            let sk = OrchardPoolTester::sk(&[0x62; 32]);
            OrchardPoolTester::sk_default_address(&sk)
        }
    };
    let account_id = wallet.get_account().id();
    let proposal = wallet
        .propose_standard_transfer::<Infallible>(
            account_id,
            StandardFeeRule::Zip317,
            ConfirmationsPolicy::new_symmetrical_unchecked(
                u32::from(scenario.confirmations),
                false,
            ),
            &recipient,
            Zatoshis::const_from_u64(scenario.payment_zat),
            None,
            None,
            scenario.change_pool.protocol(),
        )
        .map_err(|error| RunnerError::Proposal(format!("{error:?}")))?;
    snapshot_from_proposal(&proposal, "local-test").map_err(Into::into)
}

/// Exercises real wallet-owned transparent UTXOs funding a shielded recipient.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TransparentFundedPaymentScenario {
    pub funding_zat: u64,
    pub payment_zat: u64,
    pub utxo_count: u8,
    pub confirmations: u8,
    pub recipient_pool: ShieldedPoolChoice,
    pub change_pool: ShieldedPoolChoice,
}

/// Constructs transparent UTXOs in librustzcash's wallet database and invokes
/// the real transparent-input selection policy for a shielded payment.
pub fn run_transparent_funded_payment(
    scenario: &TransparentFundedPaymentScenario,
) -> Result<ProposalSnapshot, RunnerError> {
    validate_amounts(
        scenario.funding_zat,
        scenario.payment_zat,
        scenario.utxo_count,
        scenario.confirmations,
    )?;
    let mut wallet =
        TestDsl::with_sapling_birthday_account(TestDbFactory::default(), BlockCache::new())
            .map(|builder| builder.with_gap_limits(GapLimits::new(10, 5, 3)))
            .build::<OrchardPoolTester>();
    // Establish a scanned chain tip before persisting confirmed transparent UTXOs.
    let height = wallet.add_empty_blocks(usize::from(scenario.confirmations) + 2);
    let account = wallet.get_account();
    let uaddr = wallet
        .wallet()
        .get_last_generated_address_matching(account.id(), UnifiedAddressRequest::AllAvailableKeys)
        .map_err(|error| RunnerError::Proposal(format!("funding address lookup: {error:?}")))?
        .ok_or(RunnerError::InvalidScenario(
            "no generated transparent funding address",
        ))?;
    let transparent_address = uaddr.transparent().ok_or(RunnerError::InvalidScenario(
        "funding account has no transparent address",
    ))?;
    let count = u64::from(scenario.utxo_count);
    let base = scenario.funding_zat / count;
    let remainder = scenario.funding_zat % count;
    for index in 0..count {
        let mut hash = [0u8; 32];
        hash[..8].copy_from_slice(&(index + 1).to_le_bytes());
        let value = base + u64::from(index < remainder);
        let utxo = WalletTransparentOutput::from_parts(
            OutPoint::new(hash, 0),
            TxOut::new(
                Zatoshis::const_from_u64(value),
                transparent_address.script().into(),
            ),
            Some(height - u32::from(scenario.confirmations - 1)),
            Some(account.id()),
            Some(TransparentKeyScope::EXTERNAL),
            None,
        )
        .ok_or(RunnerError::InvalidScenario(
            "invalid wallet transparent UTXO",
        ))?;
        wallet
            .wallet_mut()
            .put_received_transparent_utxo(&utxo)
            .map_err(|error| {
                RunnerError::Proposal(format!("transparent funding write: {error:?}"))
            })?;
    }
    let recipient = match scenario.recipient_pool {
        ShieldedPoolChoice::Sapling => {
            let sk = SaplingPoolTester::sk(&[0x51; 32]);
            SaplingPoolTester::sk_default_address(&sk)
        }
        ShieldedPoolChoice::Orchard => {
            let sk = OrchardPoolTester::sk(&[0x52; 32]);
            OrchardPoolTester::sk_default_address(&sk)
        }
    };
    let request = TransactionRequest::new(vec![Payment::without_memo(
        recipient.to_zcash_address(wallet.network()),
        Zatoshis::const_from_u64(scenario.payment_zat),
    )])
    .map_err(|error| RunnerError::Proposal(format!("payment request: {error:?}")))?;
    let proposal = wallet
        .propose_transfer_with_policy(
            account.id(),
            &GreedyInputSelector::new(),
            &single_output_change_strategy(
                StandardFeeRule::Zip317,
                None,
                scenario.change_pool.protocol(),
            ),
            request,
            ConfirmationsPolicy::new_symmetrical_unchecked(
                u32::from(scenario.confirmations),
                false,
            ),
            &SpendPolicy::default().with_transparent(TransparentSpendPolicy::any_account_addr()),
        )
        .map_err(|error| RunnerError::Proposal(format!("{error:?}")))?;
    snapshot_from_proposal(&proposal, "local-test").map_err(Into::into)
}

/// Failure while preparing wallet state, constructing the real proposal, or
/// adapting it into ShieldSpec's stable snapshot model.
#[derive(Debug)]
pub enum RunnerError {
    InvalidScenario(&'static str),
    Proposal(String),
    Adapter(AdapterError),
}

impl fmt::Display for RunnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScenario(message) => formatter.write_str(message),
            Self::Proposal(message) => {
                write!(formatter, "Zcash proposal construction failed: {message}")
            }
            Self::Adapter(error) => {
                write!(formatter, "ShieldSpec proposal adaptation failed: {error}")
            }
        }
    }
}

impl Error for RunnerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Adapter(error) => Some(error),
            Self::InvalidScenario(_) | Self::Proposal(_) => None,
        }
    }
}

impl From<AdapterError> for RunnerError {
    fn from(value: AdapterError) -> Self {
        Self::Adapter(value)
    }
}

fn validate_amounts(
    funding_zat: u64,
    payment_zat: u64,
    note_count: u8,
    confirmations: u8,
) -> Result<(), RunnerError> {
    if note_count == 0 {
        return Err(RunnerError::InvalidScenario(
            "note_count must be greater than zero",
        ));
    }
    if note_count > 8 {
        return Err(RunnerError::InvalidScenario(
            "note_count must not exceed eight",
        ));
    }
    if confirmations == 0 {
        return Err(RunnerError::InvalidScenario(
            "confirmations must be greater than zero",
        ));
    }
    if confirmations > 10 {
        return Err(RunnerError::InvalidScenario(
            "confirmations must not exceed ten",
        ));
    }
    if funding_zat < u64::from(note_count) {
        return Err(RunnerError::InvalidScenario(
            "funding_zat must be large enough to keep every generated note nonzero",
        ));
    }
    if funding_zat == 0 {
        return Err(RunnerError::InvalidScenario(
            "funding_zat must be greater than zero",
        ));
    }
    if payment_zat == 0 {
        return Err(RunnerError::InvalidScenario(
            "payment_zat must be greater than zero",
        ));
    }
    if payment_zat >= funding_zat {
        return Err(RunnerError::InvalidScenario(
            "payment_zat must leave room for transaction fees",
        ));
    }
    Ok(())
}

/// NU6.3 activates near the synthetic wallet birthday so test proposals can
/// cross the upgrade boundary without building a long chain.
fn nu63_test_network(
    note_count: u8,
    confirmations: u8,
    empty_blocks: u8,
    activation: Nu63Activation,
) -> LocalNetwork {
    let mut network = TestBuilder::DEFAULT_NETWORK;
    let sapling_height = u32::from(
        network
            .activation_height(NetworkUpgrade::Sapling)
            .expect("test Sapling activation"),
    );
    let target_height =
        sapling_height + u32::from(note_count) + u32::from(confirmations) + u32::from(empty_blocks)
            - 1;
    let prior_upgrade = Some(BlockHeight::from_u32(sapling_height));
    network.nu6 = prior_upgrade;
    network.nu6_1 = prior_upgrade;
    network.nu6_2 = prior_upgrade;
    network.nu6_3 = Some(BlockHeight::from_u32(match activation {
        Nu63Activation::Before => target_height + 1,
        Nu63Activation::At => target_height,
    }));
    network
}

struct PaymentExecution<'a> {
    funding_zat: u64,
    payment_zat: u64,
    note_count: u8,
    confirmations: u8,
    empty_blocks: u8,
    recipient: &'a Address,
    change_pool: ShieldedPoolChoice,
    activation: Option<Nu63Activation>,
    observe_orchard_only: bool,
}

fn run_payment_with_funding<T: ShieldedPoolTester>(
    execution: PaymentExecution<'_>,
) -> Result<(ProposalSnapshot, Option<WalletPolicyDecision>), RunnerError> {
    let PaymentExecution {
        funding_zat,
        payment_zat,
        note_count,
        confirmations,
        empty_blocks,
        recipient,
        change_pool,
        activation,
        observe_orchard_only,
    } = execution;
    validate_amounts(funding_zat, payment_zat, note_count, confirmations)?;

    let gap_limits = GapLimits::new(10, 5, 3);
    let network = activation.map_or(TestBuilder::DEFAULT_NETWORK, |phase| {
        nu63_test_network(note_count, confirmations, empty_blocks, phase)
    });
    let mut wallet = TestDsl::from(
        TestBuilder::new()
            .with_network(network)
            .with_data_store_factory(TestDbFactory::default())
            .with_block_cache(BlockCache::new())
            .with_account_from_sapling_activation(BlockHash([0; 32]))
            .with_gap_limits(gap_limits),
    )
    .build::<T>();

    let note_count_u64 = u64::from(note_count);
    let base_note_value = funding_zat / note_count_u64;
    let remainder = funding_zat % note_count_u64;
    let funding_blocks = (0..note_count_u64)
        .map(|index| {
            let value = base_note_value + if index < remainder { 1 } else { 0 };
            vec![Zatoshis::const_from_u64(value)]
        })
        .collect::<Vec<_>>();
    wallet.add_notes_checking_balance(funding_blocks);

    if confirmations > 1 {
        wallet.add_empty_blocks(usize::from(confirmations - 1));
    }
    wallet.add_empty_blocks(usize::from(empty_blocks));

    let account_id = wallet.get_account().id();
    let confirmations_policy =
        ConfirmationsPolicy::new_symmetrical_unchecked(u32::from(confirmations), false);

    let proposal = wallet
        .propose_standard_transfer::<Infallible>(
            account_id,
            StandardFeeRule::Zip317,
            confirmations_policy,
            recipient,
            Zatoshis::const_from_u64(payment_zat),
            None,
            None,
            change_pool.protocol(),
        )
        .map_err(|error| RunnerError::Proposal(format!("{error:?}")))?;

    // Construct a complete, SDK-authored diagnostic proposal first. In the
    // rejection case there can be no Orchard-only proposal to snapshot, but
    // the unrestricted proposal proves that the transfer itself is fundable.
    let snapshot = snapshot_from_proposal(&proposal, "local-test")?;

    let wallet_decision = if observe_orchard_only {
        let request = TransactionRequest::new(vec![Payment::without_memo(
            recipient.to_zcash_address(wallet.network()),
            Zatoshis::const_from_u64(payment_zat),
        )])
        .map_err(|error| RunnerError::Proposal(format!("invalid policy request: {error:?}")))?;
        let selector = GreedyInputSelector::new();
        let change_strategy = SingleOutputChangeStrategy::new(
            StandardFeeRule::Zip317,
            None,
            change_pool.protocol(),
            DustOutputPolicy::default(),
        );
        let spend_policy = SpendPolicy::shielded_pools([ShieldedPool::Orchard]);

        // Only an SDK-reported shortage under the explicit spend policy is a
        // normal rejection. Sync, database, address and proposal errors must
        // remain operational failures, never fabricated policy decisions.
        let decision = match wallet.propose_transfer_with_policy(
            account_id,
            &selector,
            &change_strategy,
            request,
            confirmations_policy,
            &spend_policy,
        ) {
            Ok(_) => WalletPolicyDecision::Accepted,
            Err(WalletError::InsufficientFunds { .. }) => WalletPolicyDecision::Rejected,
            Err(error) => {
                return Err(RunnerError::Proposal(format!(
                    "Orchard-only wallet spend policy evaluation failed: {error:?}"
                )));
            }
        };
        Some(decision)
    } else {
        None
    };

    Ok((snapshot, wallet_decision))
}

fn run_orchard_payment_internal(
    scenario: &OrchardPaymentScenario,
    observe_orchard_only: bool,
    activation: Option<Nu63Activation>,
) -> Result<(ProposalSnapshot, Option<WalletPolicyDecision>), RunnerError> {
    let recipient = match scenario.recipient_pool {
        ShieldedPoolChoice::Sapling => {
            let sk = SaplingPoolTester::sk(&[scenario.recipient_seed; 32]);
            SaplingPoolTester::sk_default_address(&sk)
        }
        ShieldedPoolChoice::Orchard => {
            let sk = OrchardPoolTester::sk(&[scenario.recipient_seed; 32]);
            OrchardPoolTester::sk_default_address(&sk)
        }
    };

    match scenario.funding_pool {
        ShieldedPoolChoice::Sapling => {
            run_payment_with_funding::<SaplingPoolTester>(PaymentExecution {
                funding_zat: scenario.funding_zat,
                payment_zat: scenario.payment_zat,
                note_count: scenario.note_count,
                confirmations: scenario.confirmations,
                empty_blocks: scenario.empty_blocks,
                recipient: &recipient,
                change_pool: scenario.change_pool,
                activation,
                observe_orchard_only,
            })
        }
        ShieldedPoolChoice::Orchard => {
            run_payment_with_funding::<OrchardPoolTester>(PaymentExecution {
                funding_zat: scenario.funding_zat,
                payment_zat: scenario.payment_zat,
                note_count: scenario.note_count,
                confirmations: scenario.confirmations,
                empty_blocks: scenario.empty_blocks,
                recipient: &recipient,
                change_pool: scenario.change_pool,
                activation,
                observe_orchard_only,
            })
        }
    }
}

/// Builds an in-memory Zcash wallet and pays a real Sapling or Orchard recipient.
pub fn run_orchard_payment(
    scenario: &OrchardPaymentScenario,
) -> Result<ProposalSnapshot, RunnerError> {
    run_orchard_payment_internal(scenario, false, None).map(|(snapshot, _)| snapshot)
}

/// Run the same real wallet proposal across a specified NU6.3 activation boundary.
fn run_orchard_payment_with_activation(
    scenario: &OrchardPaymentScenario,
    activation: Option<Nu63Activation>,
) -> Result<ProposalSnapshot, RunnerError> {
    run_orchard_payment_internal(scenario, false, activation).map(|(snapshot, _)| snapshot)
}

fn run_tex_payment_internal(
    scenario: &TexPaymentScenario,
    observe_orchard_only: bool,
    activation: Option<Nu63Activation>,
) -> Result<(ProposalSnapshot, Option<WalletPolicyDecision>), RunnerError> {
    let recipient = Address::Tex([scenario.recipient_tag; 20]);

    match scenario.funding_pool {
        ShieldedPoolChoice::Sapling => {
            run_payment_with_funding::<SaplingPoolTester>(PaymentExecution {
                funding_zat: scenario.funding_zat,
                payment_zat: scenario.payment_zat,
                note_count: scenario.note_count,
                confirmations: scenario.confirmations,
                empty_blocks: scenario.empty_blocks,
                recipient: &recipient,
                change_pool: scenario.change_pool,
                activation,
                observe_orchard_only,
            })
        }
        ShieldedPoolChoice::Orchard => {
            run_payment_with_funding::<OrchardPoolTester>(PaymentExecution {
                funding_zat: scenario.funding_zat,
                payment_zat: scenario.payment_zat,
                note_count: scenario.note_count,
                confirmations: scenario.confirmations,
                empty_blocks: scenario.empty_blocks,
                recipient: &recipient,
                change_pool: scenario.change_pool,
                activation,
                observe_orchard_only,
            })
        }
    }
}

/// Runs a real ZIP 320 shielded-to-TEX proposal.
pub fn run_tex_payment(scenario: &TexPaymentScenario) -> Result<ProposalSnapshot, RunnerError> {
    run_tex_payment_internal(scenario, false, None).map(|(snapshot, _)| snapshot)
}

fn run_unified_payment_with_funding<T: ShieldedPoolTester>(
    scenario: &UnifiedPaymentScenario,
) -> Result<ProposalSnapshot, RunnerError> {
    validate_amounts(
        scenario.funding_zat,
        scenario.payment_zat,
        scenario.note_count,
        scenario.confirmations,
    )?;

    let gap_limits = GapLimits::new(10, 5, 3);
    let mut wallet =
        TestDsl::with_sapling_birthday_account(TestDbFactory::default(), BlockCache::new())
            .map(|builder| builder.with_gap_limits(gap_limits))
            .build::<T>();

    let note_count_u64 = u64::from(scenario.note_count);
    let base_note_value = scenario.funding_zat / note_count_u64;
    let remainder = scenario.funding_zat % note_count_u64;
    let funding_blocks = (0..note_count_u64)
        .map(|index| {
            let value = base_note_value + if index < remainder { 1 } else { 0 };
            vec![Zatoshis::const_from_u64(value)]
        })
        .collect::<Vec<_>>();
    wallet.add_notes_checking_balance(funding_blocks);

    if scenario.confirmations > 1 {
        wallet.add_empty_blocks(usize::from(scenario.confirmations - 1));
    }
    wallet.add_empty_blocks(usize::from(scenario.empty_blocks));

    let account = wallet
        .test_account()
        .cloned()
        .ok_or(RunnerError::InvalidScenario(
            "test wallet account is missing",
        ))?;
    let (default_ua, _) = account
        .usk()
        .to_unified_full_viewing_key()
        .default_address(UnifiedAddressRequest::AllAvailableKeys)
        .map_err(|_| RunnerError::InvalidScenario("could not derive Unified Address"))?;

    let orchard = if scenario.receivers.includes_orchard() {
        default_ua.orchard().copied()
    } else {
        None
    };
    let sapling = if scenario.receivers.includes_sapling() {
        default_ua.sapling().cloned()
    } else {
        None
    };
    let transparent = if scenario.receivers.includes_transparent() {
        default_ua.transparent().copied()
    } else {
        None
    };

    let recipient: Address =
        UnifiedAddress::from_receivers(orchard, sapling, transparent, None, None)
            .ok_or(RunnerError::InvalidScenario(
                "invalid Unified Address receiver set",
            ))?
            .into();

    let confirmations_policy =
        ConfirmationsPolicy::new_symmetrical_unchecked(u32::from(scenario.confirmations), false);

    let proposal = wallet
        .propose_standard_transfer::<Infallible>(
            account.id(),
            StandardFeeRule::Zip317,
            confirmations_policy,
            &recipient,
            Zatoshis::const_from_u64(scenario.payment_zat),
            None,
            None,
            scenario.change_pool.protocol(),
        )
        .map_err(|error| RunnerError::Proposal(format!("{error:?}")))?;

    snapshot_from_proposal(&proposal, "local-test").map_err(Into::into)
}

/// Runs a payment to a real Unified Address with a controlled receiver set.
pub fn run_unified_payment(
    scenario: &UnifiedPaymentScenario,
) -> Result<ProposalSnapshot, RunnerError> {
    match scenario.funding_pool {
        ShieldedPoolChoice::Sapling => {
            run_unified_payment_with_funding::<SaplingPoolTester>(scenario)
        }
        ShieldedPoolChoice::Orchard => {
            run_unified_payment_with_funding::<OrchardPoolTester>(scenario)
        }
    }
}

/// Stable, SDK-independent fuzzing scenario envelope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "params", rename_all = "snake_case")]
pub enum Scenario {
    OrchardPayment(OrchardPaymentScenario),
    TexPayment(TexPaymentScenario),
    UnifiedPayment(UnifiedPaymentScenario),
    TransparentFundedPayment(TransparentFundedPaymentScenario),
    Nu63OrchardPayment(Nu63OrchardPaymentScenario),
}

impl Scenario {
    #[must_use]
    pub const fn minimum_fee_margin_zat(&self) -> u64 {
        match self {
            Self::OrchardPayment(scenario) => 20_000 + (scenario.note_count as u64) * 10_000,
            Self::TexPayment(scenario) => 35_000 + (scenario.note_count as u64) * 10_000,
            Self::UnifiedPayment(scenario) => 20_000 + (scenario.note_count as u64) * 10_000,
            Self::TransparentFundedPayment(scenario) => {
                30_000 + (scenario.utxo_count as u64) * 10_000
            }
            Self::Nu63OrchardPayment(scenario) => {
                20_000 + (scenario.payment.note_count as u64) * 10_000
            }
        }
    }

    #[must_use]
    pub const fn complexity(&self) -> u128 {
        match self {
            Self::OrchardPayment(scenario) => {
                scenario.funding_zat as u128
                    + scenario.payment_zat as u128
                    + scenario.note_count as u128
                    + scenario.confirmations as u128
                    + scenario.empty_blocks as u128
                    + scenario.funding_pool.complexity_weight()
                    + scenario.change_pool.complexity_weight()
                    + scenario.recipient_pool.complexity_weight()
            }
            Self::Nu63OrchardPayment(scenario) => {
                let payment = &scenario.payment;
                payment.funding_zat as u128
                    + payment.payment_zat as u128
                    + payment.recipient_seed as u128
                    + payment.note_count as u128
                    + payment.confirmations as u128
                    + payment.empty_blocks as u128
                    + payment.funding_pool.complexity_weight()
                    + payment.change_pool.complexity_weight()
                    + 1
            }
            Self::TexPayment(scenario) => {
                scenario.funding_zat as u128
                    + scenario.payment_zat as u128
                    + scenario.recipient_tag as u128
                    + scenario.note_count as u128
                    + scenario.confirmations as u128
                    + scenario.empty_blocks as u128
                    + scenario.funding_pool.complexity_weight()
                    + scenario.change_pool.complexity_weight()
            }
            Self::TransparentFundedPayment(scenario) => {
                scenario.funding_zat as u128
                    + scenario.payment_zat as u128
                    + scenario.utxo_count as u128
                    + scenario.confirmations as u128
                    + scenario.recipient_pool.complexity_weight()
                    + scenario.change_pool.complexity_weight()
            }
            Self::UnifiedPayment(scenario) => {
                scenario.funding_zat as u128
                    + scenario.payment_zat as u128
                    + scenario.note_count as u128
                    + scenario.confirmations as u128
                    + scenario.empty_blocks as u128
                    + scenario.funding_pool.complexity_weight()
                    + scenario.change_pool.complexity_weight()
                    + scenario.receivers.complexity_weight()
            }
        }
    }
}

/// A generated scenario together with the seed and sequence position required
/// to reproduce it exactly.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedCase {
    pub seed: u64,
    pub ordinal: u64,
    pub scenario: Scenario,
}

/// Persisted minimized privacy discovery tied back to its generator origin.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Discovery {
    pub seed: u64,
    pub ordinal: u64,
    /// Optional generator family for deterministic seed/ordinal reproduction.
    /// Legacy discoveries omit the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_family: Option<String>,
    pub rule_id: String,
    pub original: Scenario,
    pub minimized: Scenario,
    /// Optional, backward-compatible exact executable provenance for
    /// differential discoveries. Old known-behavior corpus records omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<DifferentialProvenance>,
    /// Exact named wallet policy and direction, required to faithfully replay
    /// a wallet-policy discovery; absent from older snapshot-only records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet_policy_difference: Option<WalletPolicyDifference>,
}

impl Discovery {
    pub fn replay_minimized(&self) -> Result<ProposalSnapshot, RunnerError> {
        run_scenario(&self.minimized)
    }
}

/// Executes a stable fuzzing scenario against the pinned real-wallet runner.
pub fn run_scenario(scenario: &Scenario) -> Result<ProposalSnapshot, RunnerError> {
    match scenario {
        Scenario::OrchardPayment(scenario) => run_orchard_payment(scenario),
        Scenario::TexPayment(scenario) => run_tex_payment(scenario),
        Scenario::UnifiedPayment(scenario) => run_unified_payment(scenario),
        Scenario::TransparentFundedPayment(scenario) => run_transparent_funded_payment(scenario),
        Scenario::Nu63OrchardPayment(scenario) => {
            run_orchard_payment_with_activation(&scenario.payment, Some(scenario.activation))
        }
    }
}

/// Executes the SDK's Orchard-only spend selection against the same wallet state.
///
/// The snapshot records the real unrestricted diagnostic proposal. When the
/// policy rejects, no policy-conforming proposal exists to snapshot; therefore
/// the verdict is exclusively the independently observed SDK selection result.
pub fn run_scenario_with_orchard_only_spend_policy(
    scenario: &Scenario,
) -> Result<(ProposalSnapshot, WalletPolicyVerdict), RunnerError> {
    let (snapshot, decision) = match scenario {
        Scenario::OrchardPayment(scenario) => run_orchard_payment_internal(scenario, true, None),
        Scenario::TexPayment(scenario) => run_tex_payment_internal(scenario, true, None),
        Scenario::Nu63OrchardPayment(scenario) => {
            run_orchard_payment_internal(&scenario.payment, true, Some(scenario.activation))
        }
        Scenario::UnifiedPayment(_) | Scenario::TransparentFundedPayment(_) => {
            return Err(RunnerError::InvalidScenario(
                "Orchard-only spend policy is not supported for this scenario kind",
            ));
        }
    }?;
    let decision = decision.ok_or_else(|| {
        RunnerError::Proposal("Orchard-only wallet spend policy was not evaluated".to_owned())
    })?;

    Ok((
        snapshot,
        WalletPolicyVerdict {
            policy: "zcash_spend_orchard_only_v1".to_owned(),
            decision,
        },
    ))
}

/// Deterministic scenario generator. It deliberately generates semantic wallet
/// configurations instead of mutating serialized SDK objects.
#[derive(Clone, Debug)]
pub struct ScenarioGenerator {
    seed: u64,
    state: u64,
    ordinal: u64,
    transparent_funding: bool,
}

impl ScenarioGenerator {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let state = if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        };
        Self {
            seed,
            state,
            ordinal: 0,
            transparent_funding: false,
        }
    }

    #[must_use]
    pub fn new_with_transparent_funding(seed: u64) -> Self {
        Self {
            transparent_funding: true,
            ..Self::new(seed)
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.state = value;
        value.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn pick<T: Copy>(&mut self, values: &[T]) -> T {
        let index = (self.next_u64() as usize) % values.len();
        values[index]
    }

    #[must_use]
    pub fn next_case(&mut self) -> GeneratedCase {
        let ordinal = self.ordinal;
        self.ordinal += 1;

        let note_count = self.pick(&[1_u8, 1, 1, 2, 2, 3, 4]);
        let empty_blocks = self.pick(&[0_u8, 0, 0, 1, 2, 5]);
        let payment_zat = self.pick(&[10_000_u64, 25_000, 50_000, 100_000, 250_000]);
        let shape = self.next_u64();
        let confirmations = [1_u8, 1, 2, 3, 5][((shape >> 1) as usize) % 5];
        let funding_pool = if (shape & 0x10) == 0 {
            ShieldedPoolChoice::Orchard
        } else {
            ShieldedPoolChoice::Sapling
        };
        let change_pool = if (shape & 0x20) == 0 {
            funding_pool
        } else {
            funding_pool.alternate()
        };

        let scenario = if self.transparent_funding && (shape & 0x7) == 5 {
            let margin =
                30_000 + u64::from(note_count) * 10_000 + self.pick(&[0_u64, 1_000, 5_000, 10_000]);
            Scenario::TransparentFundedPayment(TransparentFundedPaymentScenario {
                funding_zat: payment_zat + margin,
                payment_zat,
                utxo_count: note_count,
                confirmations,
                recipient_pool: funding_pool,
                change_pool,
            })
        } else if matches!(shape & 0x7, 6 | 7) {
            let margin =
                20_000 + u64::from(note_count) * 10_000 + self.pick(&[0_u64, 1_000, 5_000, 10_000]);
            let receivers = self.pick(&[
                UnifiedReceiverSet::OrchardSapling,
                UnifiedReceiverSet::OrchardTransparent,
                UnifiedReceiverSet::SaplingTransparent,
                UnifiedReceiverSet::OrchardSaplingTransparent,
            ]);
            Scenario::UnifiedPayment(UnifiedPaymentScenario {
                funding_zat: payment_zat + margin,
                payment_zat,
                note_count,
                confirmations,
                empty_blocks,
                funding_pool,
                change_pool,
                receivers,
            })
        } else if shape.is_multiple_of(2) {
            let margin =
                20_000 + u64::from(note_count) * 10_000 + self.pick(&[0_u64, 1_000, 5_000, 10_000]);
            Scenario::OrchardPayment(OrchardPaymentScenario {
                funding_zat: payment_zat + margin,
                payment_zat,
                recipient_seed: self.next_u64() as u8,
                note_count,
                confirmations,
                empty_blocks,
                funding_pool,
                change_pool,
                recipient_pool: ShieldedPoolChoice::Orchard,
            })
        } else {
            let margin =
                35_000 + u64::from(note_count) * 10_000 + self.pick(&[0_u64, 1_000, 5_000, 10_000]);
            Scenario::TexPayment(TexPaymentScenario {
                funding_zat: payment_zat + margin,
                payment_zat,
                recipient_tag: self.next_u64() as u8,
                note_count,
                confirmations,
                empty_blocks,
                funding_pool,
                change_pool,
            })
        };

        GeneratedCase {
            seed: self.seed,
            ordinal,
            scenario,
        }
    }

    /// Produces an explicit Sapling-recipient scenario without changing the
    /// seed/ordinal mapping of the existing published discovery corpus.
    /// Use `sapling_recipient_scenario_at` for reproducible replay.
    #[must_use]
    pub fn next_sapling_recipient_scenario(&mut self) -> Scenario {
        let generated = self.next_case();
        match generated.scenario {
            Scenario::OrchardPayment(mut scenario) => {
                scenario.recipient_pool = ShieldedPoolChoice::Sapling;
                Scenario::OrchardPayment(scenario)
            }
            Scenario::TexPayment(source) => Scenario::OrchardPayment(OrchardPaymentScenario {
                funding_zat: source.funding_zat,
                payment_zat: source.payment_zat,
                recipient_seed: source.recipient_tag,
                note_count: source.note_count,
                confirmations: source.confirmations,
                empty_blocks: source.empty_blocks,
                funding_pool: source.funding_pool,
                change_pool: source.change_pool,
                recipient_pool: ShieldedPoolChoice::Sapling,
            }),
            Scenario::UnifiedPayment(source) => Scenario::OrchardPayment(OrchardPaymentScenario {
                funding_zat: source.funding_zat,
                payment_zat: source.payment_zat,
                recipient_seed: generated.ordinal as u8,
                note_count: source.note_count,
                confirmations: source.confirmations,
                empty_blocks: source.empty_blocks,
                funding_pool: source.funding_pool,
                change_pool: source.change_pool,
                recipient_pool: ShieldedPoolChoice::Sapling,
            }),
            // The legacy generator emits neither opt-in transparent funding nor NU6.3 cases.
            Scenario::TransparentFundedPayment(_) | Scenario::Nu63OrchardPayment(_) => {
                unreachable!("legacy generator only")
            }
        }
    }

    #[must_use]
    pub fn sapling_recipient_scenario_at(seed: u64, ordinal: u64) -> Scenario {
        let mut generator = Self::new(seed);
        for _ in 0..ordinal {
            let _ = generator.next_case();
        }
        generator.next_sapling_recipient_scenario()
    }

    #[must_use]
    pub fn case_at_with_transparent_funding(seed: u64, ordinal: u64) -> GeneratedCase {
        let mut generator = Self::new_with_transparent_funding(seed);
        for _ in 0..ordinal {
            let _ = generator.next_case();
        }
        generator.next_case()
    }

    #[must_use]
    pub fn case_at(seed: u64, ordinal: u64) -> GeneratedCase {
        let mut generator = Self::new(seed);
        for _ in 0..ordinal {
            let _ = generator.next_case();
        }
        generator.next_case()
    }

    /// Deterministic, opt-in NU6.3 proposal case with exact replay provenance.
    ///
    /// Each eight-case window covers activation phase, shielded funding pool,
    /// and preferred change pool. The SDK, not this generator, selects the
    /// actual recipient and change pools in its real proposal.
    ///
    /// The legacy `case_at` corpus and the first four NU6.3 ordinals remain
    /// unchanged, so existing seed/ordinal discovery records still replay.
    #[must_use]
    pub fn nu63_case_at(seed: u64, ordinal: u64) -> GeneratedCase {
        let generated = Self::case_at(seed, ordinal);
        let mut payment = match generated.scenario {
            Scenario::OrchardPayment(payment) => payment,
            Scenario::TexPayment(tex) => OrchardPaymentScenario {
                funding_zat: tex.funding_zat,
                payment_zat: tex.payment_zat,
                recipient_seed: tex.recipient_tag,
                note_count: tex.note_count,
                confirmations: tex.confirmations,
                empty_blocks: tex.empty_blocks,
                funding_pool: tex.funding_pool,
                change_pool: tex.change_pool,
                recipient_pool: ShieldedPoolChoice::Orchard,
            },
            Scenario::UnifiedPayment(unified) => OrchardPaymentScenario {
                funding_zat: unified.funding_zat,
                payment_zat: unified.payment_zat,
                recipient_seed: ordinal as u8,
                note_count: unified.note_count,
                confirmations: unified.confirmations,
                empty_blocks: unified.empty_blocks,
                funding_pool: unified.funding_pool,
                change_pool: unified.change_pool,
                recipient_pool: ShieldedPoolChoice::Orchard,
            },
            Scenario::TransparentFundedPayment(_) => {
                unreachable!("legacy generator does not emit transparent funding")
            }
            Scenario::Nu63OrchardPayment(_) => {
                unreachable!("legacy generator emits only legacy cases")
            }
        };
        let funding_pool = if ordinal % 4 < 2 {
            ShieldedPoolChoice::Orchard
        } else {
            ShieldedPoolChoice::Sapling
        };
        payment.funding_pool = funding_pool;
        payment.change_pool = if ordinal % 8 < 4 {
            funding_pool
        } else {
            funding_pool.alternate()
        };
        GeneratedCase {
            seed,
            ordinal,
            scenario: Scenario::Nu63OrchardPayment(Nu63OrchardPaymentScenario {
                payment,
                activation: if ordinal.is_multiple_of(2) {
                    Nu63Activation::At
                } else {
                    Nu63Activation::Before
                },
            }),
        }
    }

    /// Opt-in activation-aware proposal, sharing the replay contract above.
    #[must_use]
    pub fn nu63_scenario_at(seed: u64, ordinal: u64) -> Scenario {
        Self::nu63_case_at(seed, ordinal).scenario
    }
}

fn shrink_candidates(scenario: &Scenario) -> Vec<Scenario> {
    let mut candidates = Vec::new();

    match scenario {
        Scenario::TransparentFundedPayment(current) => {
            if current.confirmations > 1 {
                let mut candidate = current.clone();
                candidate.confirmations = 1;
                candidates.push(Scenario::TransparentFundedPayment(candidate));
            }
            if current.utxo_count > 1 {
                let mut candidate = current.clone();
                candidate.utxo_count = 1;
                candidates.push(Scenario::TransparentFundedPayment(candidate));
            }
            if current.recipient_pool != ShieldedPoolChoice::Orchard {
                let mut candidate = current.clone();
                candidate.recipient_pool = ShieldedPoolChoice::Orchard;
                candidates.push(Scenario::TransparentFundedPayment(candidate));
            }
            if current.change_pool != ShieldedPoolChoice::Orchard {
                let mut candidate = current.clone();
                candidate.change_pool = ShieldedPoolChoice::Orchard;
                candidates.push(Scenario::TransparentFundedPayment(candidate));
            }
            for payment_zat in [1, current.payment_zat / 2] {
                if payment_zat > 0 && payment_zat < current.payment_zat {
                    let mut candidate = current.clone();
                    candidate.payment_zat = payment_zat;
                    candidates.push(Scenario::TransparentFundedPayment(candidate));
                }
            }
            let minimum_funding = current
                .payment_zat
                .saturating_add(30_000 + u64::from(current.utxo_count) * 10_000);
            if minimum_funding < current.funding_zat {
                let mut candidate = current.clone();
                candidate.funding_zat = minimum_funding;
                candidates.push(Scenario::TransparentFundedPayment(candidate));
            }
        }
        Scenario::OrchardPayment(current) => {
            if current.confirmations != 1 {
                let mut candidate = current.clone();
                candidate.confirmations = 1;
                candidates.push(Scenario::OrchardPayment(candidate));
            }
            if current.funding_pool != ShieldedPoolChoice::Orchard {
                let mut candidate = current.clone();
                candidate.funding_pool = ShieldedPoolChoice::Orchard;
                candidates.push(Scenario::OrchardPayment(candidate));
            }
            if current.change_pool != ShieldedPoolChoice::Orchard {
                let mut candidate = current.clone();
                candidate.change_pool = ShieldedPoolChoice::Orchard;
                candidates.push(Scenario::OrchardPayment(candidate));
            }
            if current.empty_blocks != 0 {
                let mut candidate = current.clone();
                candidate.empty_blocks = 0;
                candidates.push(Scenario::OrchardPayment(candidate));
            }
            if current.note_count != 1 {
                let mut candidate = current.clone();
                candidate.note_count = 1;
                candidates.push(Scenario::OrchardPayment(candidate));
            }
            if current.recipient_seed != 0 {
                let mut candidate = current.clone();
                candidate.recipient_seed = 0;
                candidates.push(Scenario::OrchardPayment(candidate));
            }

            for payment_zat in [1, current.payment_zat / 2] {
                if payment_zat > 0 && payment_zat < current.payment_zat {
                    let mut candidate = current.clone();
                    candidate.payment_zat = payment_zat;
                    candidate.funding_zat = candidate.funding_zat.max(
                        payment_zat
                            .saturating_add(20_000 + u64::from(candidate.note_count) * 10_000),
                    );
                    candidates.push(Scenario::OrchardPayment(candidate));
                }
            }

            let minimum_funding = current
                .payment_zat
                .saturating_add(20_000 + u64::from(current.note_count) * 10_000);
            for funding_zat in [
                minimum_funding,
                minimum_funding + current.funding_zat.saturating_sub(minimum_funding) / 2,
            ] {
                if funding_zat < current.funding_zat {
                    let mut candidate = current.clone();
                    candidate.funding_zat = funding_zat;
                    candidates.push(Scenario::OrchardPayment(candidate));
                }
            }
        }
        Scenario::Nu63OrchardPayment(current) => {
            for candidate in shrink_candidates(&Scenario::OrchardPayment(current.payment.clone())) {
                if let Scenario::OrchardPayment(payment) = candidate {
                    candidates.push(Scenario::Nu63OrchardPayment(Nu63OrchardPaymentScenario {
                        payment,
                        activation: current.activation,
                    }));
                }
            }
        }
        Scenario::TexPayment(current) => {
            if current.confirmations != 1 {
                let mut candidate = current.clone();
                candidate.confirmations = 1;
                candidates.push(Scenario::TexPayment(candidate));
            }
            if current.funding_pool != ShieldedPoolChoice::Orchard {
                let mut candidate = current.clone();
                candidate.funding_pool = ShieldedPoolChoice::Orchard;
                candidates.push(Scenario::TexPayment(candidate));
            }
            if current.change_pool != ShieldedPoolChoice::Orchard {
                let mut candidate = current.clone();
                candidate.change_pool = ShieldedPoolChoice::Orchard;
                candidates.push(Scenario::TexPayment(candidate));
            }
            if current.empty_blocks != 0 {
                let mut candidate = current.clone();
                candidate.empty_blocks = 0;
                candidates.push(Scenario::TexPayment(candidate));
            }
            if current.note_count != 1 {
                let mut candidate = current.clone();
                candidate.note_count = 1;
                candidates.push(Scenario::TexPayment(candidate));
            }
            if current.recipient_tag != 0 {
                let mut candidate = current.clone();
                candidate.recipient_tag = 0;
                candidates.push(Scenario::TexPayment(candidate));
            }

            for payment_zat in [1, current.payment_zat / 2] {
                if payment_zat > 0 && payment_zat < current.payment_zat {
                    let mut candidate = current.clone();
                    candidate.payment_zat = payment_zat;
                    candidate.funding_zat = candidate.funding_zat.max(
                        payment_zat
                            .saturating_add(35_000 + u64::from(candidate.note_count) * 10_000),
                    );
                    candidates.push(Scenario::TexPayment(candidate));
                }
            }

            let minimum_funding = current
                .payment_zat
                .saturating_add(35_000 + u64::from(current.note_count) * 10_000);
            for funding_zat in [
                minimum_funding,
                minimum_funding + current.funding_zat.saturating_sub(minimum_funding) / 2,
            ] {
                if funding_zat < current.funding_zat {
                    let mut candidate = current.clone();
                    candidate.funding_zat = funding_zat;
                    candidates.push(Scenario::TexPayment(candidate));
                }
            }
        }
        Scenario::UnifiedPayment(current) => {
            if current.confirmations != 1 {
                let mut candidate = current.clone();
                candidate.confirmations = 1;
                candidates.push(Scenario::UnifiedPayment(candidate));
            }
            if current.funding_pool != ShieldedPoolChoice::Orchard {
                let mut candidate = current.clone();
                candidate.funding_pool = ShieldedPoolChoice::Orchard;
                candidates.push(Scenario::UnifiedPayment(candidate));
            }
            if current.change_pool != ShieldedPoolChoice::Orchard {
                let mut candidate = current.clone();
                candidate.change_pool = ShieldedPoolChoice::Orchard;
                candidates.push(Scenario::UnifiedPayment(candidate));
            }
            if current.empty_blocks != 0 {
                let mut candidate = current.clone();
                candidate.empty_blocks = 0;
                candidates.push(Scenario::UnifiedPayment(candidate));
            }
            if current.note_count != 1 {
                let mut candidate = current.clone();
                candidate.note_count = 1;
                candidates.push(Scenario::UnifiedPayment(candidate));
            }
            if current.receivers == UnifiedReceiverSet::OrchardSaplingTransparent {
                for receivers in [
                    UnifiedReceiverSet::OrchardTransparent,
                    UnifiedReceiverSet::SaplingTransparent,
                    UnifiedReceiverSet::OrchardSapling,
                ] {
                    let mut candidate = current.clone();
                    candidate.receivers = receivers;
                    candidates.push(Scenario::UnifiedPayment(candidate));
                }
            }

            for payment_zat in [1, current.payment_zat / 2] {
                if payment_zat > 0 && payment_zat < current.payment_zat {
                    let mut candidate = current.clone();
                    candidate.payment_zat = payment_zat;
                    candidate.funding_zat = candidate.funding_zat.max(
                        payment_zat
                            .saturating_add(20_000 + u64::from(candidate.note_count) * 10_000),
                    );
                    candidates.push(Scenario::UnifiedPayment(candidate));
                }
            }

            let minimum_funding = current
                .payment_zat
                .saturating_add(20_000 + u64::from(current.note_count) * 10_000);
            for funding_zat in [
                minimum_funding,
                minimum_funding + current.funding_zat.saturating_sub(minimum_funding) / 2,
            ] {
                if funding_zat < current.funding_zat {
                    let mut candidate = current.clone();
                    candidate.funding_zat = funding_zat;
                    candidates.push(Scenario::UnifiedPayment(candidate));
                }
            }
        }
    }

    candidates
}

/// Greedily minimizes a scenario while preserving a caller-defined failure.
///
/// The predicate can execute the real proposal runner plus ShieldSpec policy
/// evaluation, so minimized discoveries remain executable counterexamples.
#[must_use]
pub fn shrink_scenario(
    scenario: &Scenario,
    mut still_fails: impl FnMut(&Scenario) -> bool,
) -> Scenario {
    let mut current = scenario.clone();

    loop {
        let mut improved = false;
        let current_complexity = current.complexity();

        for candidate in shrink_candidates(&current) {
            if candidate.complexity() < current_complexity && still_fails(&candidate) {
                current = candidate;
                improved = true;
                break;
            }
        }

        if !improved {
            return current;
        }
    }
}
