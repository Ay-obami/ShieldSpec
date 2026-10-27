//! Deterministic real-wallet proposal runner for ShieldSpec fuzzing.
//!
//! This tool deliberately uses librustzcash's downstream test harness to create
//! realistic wallet state and invoke the real proposal machinery. It is kept
//! outside the production workspace because it depends on test-only SDK APIs.

use std::{convert::Infallible, error::Error, fmt};

use serde::{Deserialize, Serialize};
use shieldspec_core::ProposalSnapshot;
use shieldspec_zcash::{snapshot_from_proposal, AdapterError};
use zcash_client_backend::{
    data_api::{
        testing::{
            orchard::OrchardPoolTester,
            pool::{dsl::TestDsl, ShieldedPoolTester},
        },
        wallet::ConfirmationsPolicy,
        Account,
    },
    fees::StandardFeeRule,
};
use zcash_client_sqlite::testing::{db::TestDbFactory, BlockCache};
use zcash_keys::{address::Address, keys::transparent::gap_limits::GapLimits};
use zcash_protocol::{value::Zatoshis, ShieldedPool};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchardPaymentScenario {
    pub funding_zat: u64,
    pub payment_zat: u64,
    pub recipient_seed: u8,
}

impl OrchardPaymentScenario {
    #[must_use]
    pub const fn demo() -> Self {
        Self {
            funding_zat: 120_000,
            payment_zat: 100_000,
            recipient_seed: 0x42,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TexPaymentScenario {
    pub funding_zat: u64,
    pub payment_zat: u64,
    pub recipient_tag: u8,
}

impl TexPaymentScenario {
    #[must_use]
    pub const fn demo() -> Self {
        Self {
            funding_zat: 100_000,
            payment_zat: 50_000,
            recipient_tag: 0x04,
        }
    }
}

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

fn validate_amounts(funding_zat: u64, payment_zat: u64) -> Result<(), RunnerError> {
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

/// Funds an in-memory Orchard wallet through a synthetic compact block, scans
/// it, invokes the real librustzcash proposal engine, and normalizes the real
/// proposal through ShieldSpec.
pub fn run_orchard_payment(
    scenario: &OrchardPaymentScenario,
) -> Result<ProposalSnapshot, RunnerError> {
    validate_amounts(scenario.funding_zat, scenario.payment_zat)?;

    let mut wallet =
        TestDsl::with_sapling_birthday_account(TestDbFactory::default(), BlockCache::new())
            .build::<OrchardPoolTester>();

    wallet.add_a_single_note_checking_balance(Zatoshis::const_from_u64(scenario.funding_zat));

    let recipient_sk = OrchardPoolTester::sk(&[scenario.recipient_seed; 32]);
    let recipient = OrchardPoolTester::sk_default_address(&recipient_sk);
    let account_id = wallet.get_account().id();

    let proposal = wallet
        .propose_standard_transfer::<Infallible>(
            account_id,
            StandardFeeRule::Zip317,
            ConfirmationsPolicy::MIN,
            &recipient,
            Zatoshis::const_from_u64(scenario.payment_zat),
            None,
            None,
            ShieldedPool::Orchard,
        )
        .map_err(|error| RunnerError::Proposal(format!("{error:?}")))?;

    snapshot_from_proposal(&proposal, "local-test").map_err(Into::into)
}

/// Runs the real ZIP 320 shielded-to-TEX proposal path. librustzcash creates
/// the ephemeral transparent bridge and prior-step reference itself.
pub fn run_tex_payment(scenario: &TexPaymentScenario) -> Result<ProposalSnapshot, RunnerError> {
    validate_amounts(scenario.funding_zat, scenario.payment_zat)?;

    let gap_limits = GapLimits::new(10, 5, 3);
    let mut wallet =
        TestDsl::with_sapling_birthday_account(TestDbFactory::default(), BlockCache::new())
            .map(|builder| builder.with_gap_limits(gap_limits))
            .build::<OrchardPoolTester>();

    wallet.add_a_single_note_checking_balance(Zatoshis::const_from_u64(scenario.funding_zat));

    let recipient = Address::Tex([scenario.recipient_tag; 20]);
    let account_id = wallet.get_account().id();

    let proposal = wallet
        .propose_standard_transfer::<Infallible>(
            account_id,
            StandardFeeRule::Zip317,
            ConfirmationsPolicy::MIN,
            &recipient,
            Zatoshis::const_from_u64(scenario.payment_zat),
            None,
            None,
            ShieldedPool::Orchard,
        )
        .map_err(|error| RunnerError::Proposal(format!("{error:?}")))?;

    snapshot_from_proposal(&proposal, "local-test").map_err(Into::into)
}
