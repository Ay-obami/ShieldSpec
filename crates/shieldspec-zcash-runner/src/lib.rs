//! Deterministic real-wallet proposal runner for ShieldSpec fuzzing.
//!
//! This crate deliberately uses librustzcash's downstream test harness to create
//! realistic wallet state and invoke the real proposal machinery. It is not
//! published as a production dependency.

use std::{convert::Infallible, error::Error, fmt};

use serde::{Deserialize, Serialize};
use shieldspec_core::ProposalSnapshot;
use shieldspec_zcash::{snapshot_from_proposal, AdapterError};
use zcash_client_backend::{
    data_api::{
        testing::single_output_change_strategy,
        testing::{
            orchard::OrchardPoolTester,
            pool::{dsl::TestDsl, ShieldedPoolTester},
            sapling::SaplingPoolTester,
        },
        wallet::input_selection::{GreedyInputSelector, SpendPolicy, TransparentSpendPolicy},
        wallet::ConfirmationsPolicy,
        Account, WalletRead, WalletWrite,
    },
    fees::StandardFeeRule,
    wallet::WalletTransparentOutput,
};
use zcash_client_sqlite::testing::{db::TestDbFactory, BlockCache};
use zcash_keys::{
    address::{Address, UnifiedAddress},
    keys::{transparent::gap_limits::GapLimits, UnifiedAddressRequest},
};
use zcash_protocol::{value::Zatoshis, ShieldedPool};
use zcash_transparent::{
    bundle::{OutPoint, TxOut},
    keys::TransparentKeyScope,
};
use zip321::{Payment, TransactionRequest};

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

fn run_payment_with_funding<T: ShieldedPoolTester>(
    funding_zat: u64,
    payment_zat: u64,
    note_count: u8,
    confirmations: u8,
    empty_blocks: u8,
    recipient: &Address,
    change_pool: ShieldedPoolChoice,
) -> Result<ProposalSnapshot, RunnerError> {
    validate_amounts(funding_zat, payment_zat, note_count, confirmations)?;

    let gap_limits = GapLimits::new(10, 5, 3);
    let mut wallet =
        TestDsl::with_sapling_birthday_account(TestDbFactory::default(), BlockCache::new())
            .map(|builder| builder.with_gap_limits(gap_limits))
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

    snapshot_from_proposal(&proposal, "local-test").map_err(Into::into)
}

/// Builds an in-memory Zcash wallet and pays a real Sapling or Orchard recipient.
pub fn run_orchard_payment(
    scenario: &OrchardPaymentScenario,
) -> Result<ProposalSnapshot, RunnerError> {
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
        ShieldedPoolChoice::Sapling => run_payment_with_funding::<SaplingPoolTester>(
            scenario.funding_zat,
            scenario.payment_zat,
            scenario.note_count,
            scenario.confirmations,
            scenario.empty_blocks,
            &recipient,
            scenario.change_pool,
        ),
        ShieldedPoolChoice::Orchard => run_payment_with_funding::<OrchardPoolTester>(
            scenario.funding_zat,
            scenario.payment_zat,
            scenario.note_count,
            scenario.confirmations,
            scenario.empty_blocks,
            &recipient,
            scenario.change_pool,
        ),
    }
}

/// Runs a real ZIP 320 shielded-to-TEX proposal.
pub fn run_tex_payment(scenario: &TexPaymentScenario) -> Result<ProposalSnapshot, RunnerError> {
    let recipient = Address::Tex([scenario.recipient_tag; 20]);

    match scenario.funding_pool {
        ShieldedPoolChoice::Sapling => run_payment_with_funding::<SaplingPoolTester>(
            scenario.funding_zat,
            scenario.payment_zat,
            scenario.note_count,
            scenario.confirmations,
            scenario.empty_blocks,
            &recipient,
            scenario.change_pool,
        ),
        ShieldedPoolChoice::Orchard => run_payment_with_funding::<OrchardPoolTester>(
            scenario.funding_zat,
            scenario.payment_zat,
            scenario.note_count,
            scenario.confirmations,
            scenario.empty_blocks,
            &recipient,
            scenario.change_pool,
        ),
    }
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
}

impl Scenario {
    #[must_use]
    pub const fn minimum_fee_margin_zat(&self) -> u64 {
        match self {
            Self::OrchardPayment(scenario) => 20_000 + (scenario.note_count as u64) * 10_000,
            Self::TexPayment(scenario) => 35_000 + (scenario.note_count as u64) * 10_000,
            Self::UnifiedPayment(scenario) => 20_000 + (scenario.note_count as u64) * 10_000,
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
    pub rule_id: String,
    pub original: Scenario,
    pub minimized: Scenario,
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
    }
}

/// Deterministic scenario generator. It deliberately generates semantic wallet
/// configurations instead of mutating serialized SDK objects.
#[derive(Clone, Debug)]
pub struct ScenarioGenerator {
    seed: u64,
    state: u64,
    ordinal: u64,
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

        let scenario = if matches!(shape & 0x7, 6 | 7) {
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
        let mut generated = self.next_case();
        if let Scenario::OrchardPayment(scenario) = &mut generated.scenario {
            scenario.recipient_pool = ShieldedPoolChoice::Sapling;
        } else {
            let source = match generated.scenario {
                Scenario::TexPayment(ref scenario) => scenario,
                Scenario::OrchardPayment(_) => unreachable!(),
            };
            generated.scenario = Scenario::OrchardPayment(OrchardPaymentScenario {
                funding_zat: source.funding_zat,
                payment_zat: source.payment_zat,
                recipient_seed: source.recipient_tag,
                note_count: source.note_count,
                confirmations: source.confirmations,
                empty_blocks: source.empty_blocks,
                funding_pool: source.funding_pool,
                change_pool: source.change_pool,
                recipient_pool: ShieldedPoolChoice::Sapling,
            });
        }
        generated.scenario
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
    pub fn case_at(seed: u64, ordinal: u64) -> GeneratedCase {
        let mut generator = Self::new(seed);
        for _ in 0..ordinal {
            let _ = generator.next_case();
        }
        generator.next_case()
    }
}

fn shrink_candidates(scenario: &Scenario) -> Vec<Scenario> {
    let mut candidates = Vec::new();

    match scenario {
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
