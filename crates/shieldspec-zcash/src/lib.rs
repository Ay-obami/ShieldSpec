//! Keyless, networkless adaptation of Zcash SDK proposals into ShieldSpec snapshots.

use std::{error::Error, fmt};

use shieldspec_core::{InputFact, OutputFact, OutputRole, Pool, ProposalSnapshot, StepSnapshot};
use zcash_client_backend::proposal::{Proposal, StepOutputIndex};
use zcash_protocol::{PoolType, ShieldedPool};

/// A proposal fact could not be represented safely in a ShieldSpec snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterError {
    /// A payment has no amount, so its value cannot be recorded.
    MissingPaymentAmount {
        step_index: usize,
        payment_index: usize,
    },
    /// A payment has no SDK-selected output pool.
    MissingSelectedPool {
        step_index: usize,
        payment_index: usize,
    },
    /// A prior-step input does not refer to an earlier proposal step.
    InvalidPriorStep {
        step_index: usize,
        referenced_step: usize,
    },
    /// A prior-step input refers to a payment that cannot be resolved.
    InvalidPriorPayment {
        step_index: usize,
        referenced_step: usize,
        payment_index: usize,
    },
    /// A prior-step input refers to a change output that cannot be resolved.
    InvalidPriorChange {
        step_index: usize,
        referenced_step: usize,
        change_index: usize,
    },
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPaymentAmount {
                step_index,
                payment_index,
            } => write!(
                formatter,
                "payment {payment_index} in step {step_index} has no amount"
            ),
            Self::MissingSelectedPool {
                step_index,
                payment_index,
            } => write!(
                formatter,
                "payment {payment_index} in step {step_index} has no selected pool"
            ),
            Self::InvalidPriorStep {
                step_index,
                referenced_step,
            } => write!(
                formatter,
                "step {step_index} has an invalid reference to step {referenced_step}"
            ),
            Self::InvalidPriorPayment {
                step_index,
                referenced_step,
                payment_index,
            } => write!(
                formatter,
                "step {step_index} references missing payment {payment_index} in step {referenced_step}"
            ),
            Self::InvalidPriorChange {
                step_index,
                referenced_step,
                change_index,
            } => write!(
                formatter,
                "step {step_index} references missing change {change_index} in step {referenced_step}"
            ),
        }
    }
}

impl Error for AdapterError {}

const fn snapshot_pool(pool: PoolType) -> Pool {
    match pool {
        PoolType::Transparent => Pool::Transparent,
        PoolType::Shielded(ShieldedPool::Sapling) => Pool::Sapling,
        PoolType::Shielded(ShieldedPool::Orchard) => Pool::Orchard,
        PoolType::Shielded(ShieldedPool::Ironwood) => Pool::Ironwood,
    }
}

/// Converts every SDK proposal step into a normalized privacy snapshot.
///
/// The proposal contains no network identifier, so `network` must be supplied
/// by the caller. This function only inspects proposal data; it requires no
/// wallet keys, database, or network connection.
pub fn snapshot_from_proposal<FeeRule, NoteRef>(
    proposal: &Proposal<FeeRule, NoteRef>,
    network: impl Into<String>,
) -> Result<ProposalSnapshot, AdapterError> {
    let mut snapshots = Vec::with_capacity(proposal.steps().len());

    for (step_index, step) in proposal.steps().iter().enumerate() {
        let mut inputs = Vec::new();
        inputs.extend(step.transparent_inputs().iter().map(|input| InputFact {
            pool: Pool::Transparent,
            value_zat: input.txout().value().into_u64(),
        }));
        inputs.extend(
            step.shielded_inputs()
                .into_iter()
                .flat_map(|shielded| shielded.notes())
                .map(|received| InputFact {
                    pool: snapshot_pool(PoolType::Shielded(received.note().pool())),
                    value_zat: received.note().value().into_u64(),
                }),
        );

        for prior_input in step.prior_step_inputs() {
            let referenced_step = prior_input.step_index();
            let source = proposal
                .steps()
                .get(referenced_step)
                .filter(|_| referenced_step < step_index)
                .ok_or(AdapterError::InvalidPriorStep {
                    step_index,
                    referenced_step,
                })?;
            let (pool, value) = match prior_input.output_index() {
                StepOutputIndex::Payment(payment_index) => {
                    let pool = source.payment_pools().get(&payment_index).copied().ok_or(
                        AdapterError::InvalidPriorPayment {
                            step_index,
                            referenced_step,
                            payment_index,
                        },
                    )?;
                    let value = source
                        .transaction_request()
                        .payments()
                        .get(&payment_index)
                        .and_then(|payment| payment.amount())
                        .ok_or(AdapterError::InvalidPriorPayment {
                            step_index,
                            referenced_step,
                            payment_index,
                        })?;
                    (pool, value)
                }
                StepOutputIndex::Change(change_index) => {
                    let change = source.balance().proposed_change().get(change_index).ok_or(
                        AdapterError::InvalidPriorChange {
                            step_index,
                            referenced_step,
                            change_index,
                        },
                    )?;
                    (change.output_pool(), change.value())
                }
            };
            inputs.push(InputFact {
                pool: snapshot_pool(pool),
                value_zat: value.into_u64(),
            });
        }

        let mut outputs = Vec::with_capacity(
            step.transaction_request().payments().len() + step.balance().proposed_change().len(),
        );
        for (&payment_index, payment) in step.transaction_request().payments() {
            let pool = step.payment_pools().get(&payment_index).copied().ok_or(
                AdapterError::MissingSelectedPool {
                    step_index,
                    payment_index,
                },
            )?;
            let value = payment.amount().ok_or(AdapterError::MissingPaymentAmount {
                step_index,
                payment_index,
            })?;
            outputs.push(OutputFact {
                pool: snapshot_pool(pool),
                role: OutputRole::Recipient,
                value_zat: value.into_u64(),
            });
        }
        outputs.extend(
            step.balance()
                .proposed_change()
                .iter()
                .map(|change| OutputFact {
                    pool: snapshot_pool(change.output_pool()),
                    role: OutputRole::Change,
                    value_zat: change.value().into_u64(),
                }),
        );

        snapshots.push(StepSnapshot {
            index: step_index,
            inputs,
            outputs,
            fee_zat: step.balance().fee_required().into_u64(),
        });
    }

    Ok(ProposalSnapshot {
        schema_version: 1,
        network: network.into(),
        steps: snapshots,
    })
}
