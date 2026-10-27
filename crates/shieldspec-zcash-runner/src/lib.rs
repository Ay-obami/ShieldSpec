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
        testing::{
            orchard::OrchardPoolTester,
            pool::{dsl::TestDsl, ShieldedPoolTester},
            sapling::SaplingPoolTester,
            TestBuilder,
        },
        wallet::{
            input_selection::{GreedyInputSelector, SpendPolicy},
            ConfirmationsPolicy,
        },
        Account,
    },
    fees::{standard::SingleOutputChangeStrategy, DustOutputPolicy, StandardFeeRule},
    zip321::{Payment, TransactionRequest},
};
use zcash_client_sqlite::testing::{db::TestDbFactory, BlockCache};
use zcash_keys::{address::Address, keys::transparent::gap_limits::GapLimits};
use zcash_primitives::block::BlockHash;
use zcash_protocol::{
    consensus::{BlockHeight, NetworkUpgrade, Parameters},
    local_consensus::LocalNetwork,
    value::Zatoshis,
    ShieldedPool,
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
    let recipient_sk = OrchardPoolTester::sk(&[scenario.recipient_seed; 32]);
    let recipient = OrchardPoolTester::sk_default_address(&recipient_sk);

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

/// Builds an in-memory Zcash wallet and pays an Orchard recipient.
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

/// Stable, SDK-independent fuzzing scenario envelope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "params", rename_all = "snake_case")]
pub enum Scenario {
    OrchardPayment(OrchardPaymentScenario),
    TexPayment(TexPaymentScenario),
    Nu63OrchardPayment(Nu63OrchardPaymentScenario),
}

impl Scenario {
    #[must_use]
    pub const fn minimum_fee_margin_zat(&self) -> u64 {
        match self {
            Self::OrchardPayment(scenario) => 20_000 + (scenario.note_count as u64) * 10_000,
            Self::TexPayment(scenario) => 35_000 + (scenario.note_count as u64) * 10_000,
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
                    + scenario.recipient_seed as u128
                    + scenario.note_count as u128
                    + scenario.confirmations as u128
                    + scenario.empty_blocks as u128
                    + scenario.funding_pool.complexity_weight()
                    + scenario.change_pool.complexity_weight()
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

        let scenario = if shape.is_multiple_of(2) {
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

    #[must_use]
    pub fn case_at(seed: u64, ordinal: u64) -> GeneratedCase {
        let mut generator = Self::new(seed);
        for _ in 0..ordinal {
            let _ = generator.next_case();
        }
        generator.next_case()
    }

    /// Deterministic, opt-in NU6.3 proposal generation. It deliberately does
    /// not alter the legacy `case_at` seed/ordinal replay contract.
    #[must_use]
    pub fn nu63_scenario_at(seed: u64, ordinal: u64) -> Scenario {
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
            },
            Scenario::Nu63OrchardPayment(_) => {
                unreachable!("legacy generator emits only legacy cases")
            }
        };
        payment.funding_pool = ShieldedPoolChoice::Orchard;
        payment.change_pool = ShieldedPoolChoice::Orchard;
        Scenario::Nu63OrchardPayment(Nu63OrchardPaymentScenario {
            payment,
            activation: if ordinal.is_multiple_of(2) {
                Nu63Activation::At
            } else {
                Nu63Activation::Before
            },
        })
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
