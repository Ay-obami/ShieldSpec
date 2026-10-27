use std::error::Error;
use std::fmt;

use crate::ProposalSnapshot;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotValidationError {
    UnsupportedSchemaVersion {
        actual: u32,
    },
    EmptyNetwork,
    NoSteps,
    InvalidStepIndex {
        position: usize,
        actual: usize,
    },
    NoInputs {
        step_index: usize,
    },
    NoOutputs {
        step_index: usize,
    },
    ArithmeticOverflow {
        step_index: usize,
    },
    UnbalancedStep {
        step_index: usize,
        input_total_zat: u128,
        output_and_fee_total_zat: u128,
    },
}

impl fmt::Display for SnapshotValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { actual } => {
                write!(formatter, "unsupported schema version {actual}; expected 1")
            }
            Self::EmptyNetwork => formatter.write_str("network must not be empty"),
            Self::NoSteps => formatter.write_str("proposal must contain at least one step"),
            Self::InvalidStepIndex { position, actual } => write!(
                formatter,
                "step at position {position} has index {actual}; expected {position}"
            ),
            Self::NoInputs { step_index } => {
                write!(formatter, "step {step_index} must contain at least one input")
            }
            Self::NoOutputs { step_index } => {
                write!(formatter, "step {step_index} must contain at least one output")
            }
            Self::ArithmeticOverflow { step_index } => {
                write!(formatter, "value total overflows at step {step_index}")
            }
            Self::UnbalancedStep {
                step_index,
                input_total_zat,
                output_and_fee_total_zat,
            } => write!(
                formatter,
                "step {step_index} is unbalanced: inputs total {input_total_zat} zat but outputs plus fee total {output_and_fee_total_zat} zat"
            ),
        }
    }
}

impl Error for SnapshotValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComparisonContextError {
    SchemaVersionMismatch { baseline: u32, candidate: u32 },
    NetworkMismatch { baseline: String, candidate: String },
}

impl fmt::Display for ComparisonContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaVersionMismatch {
                baseline,
                candidate,
            } => write!(
                formatter,
                "baseline schema version {baseline} does not match candidate schema version {candidate}"
            ),
            Self::NetworkMismatch {
                baseline,
                candidate,
            } => write!(
                formatter,
                "baseline network {baseline:?} does not match candidate network {candidate:?}"
            ),
        }
    }
}

impl Error for ComparisonContextError {}

pub fn validate_snapshot(snapshot: &ProposalSnapshot) -> Result<(), SnapshotValidationError> {
    if snapshot.schema_version != 1 {
        return Err(SnapshotValidationError::UnsupportedSchemaVersion {
            actual: snapshot.schema_version,
        });
    }
    if snapshot.network.trim().is_empty() {
        return Err(SnapshotValidationError::EmptyNetwork);
    }
    if snapshot.steps.is_empty() {
        return Err(SnapshotValidationError::NoSteps);
    }

    for (position, step) in snapshot.steps.iter().enumerate() {
        if step.index != position {
            return Err(SnapshotValidationError::InvalidStepIndex {
                position,
                actual: step.index,
            });
        }
        if step.inputs.is_empty() {
            return Err(SnapshotValidationError::NoInputs {
                step_index: step.index,
            });
        }
        if step.outputs.is_empty() {
            return Err(SnapshotValidationError::NoOutputs {
                step_index: step.index,
            });
        }

        let input_total_zat =
            checked_sum(step.inputs.iter().map(|input| input.value_zat), step.index)?;
        let output_total_zat = checked_sum(
            step.outputs.iter().map(|output| output.value_zat),
            step.index,
        )?;
        let output_and_fee_total_zat = output_total_zat
            .checked_add(u128::from(step.fee_zat))
            .ok_or(SnapshotValidationError::ArithmeticOverflow {
                step_index: step.index,
            })?;
        if input_total_zat != output_and_fee_total_zat {
            return Err(SnapshotValidationError::UnbalancedStep {
                step_index: step.index,
                input_total_zat,
                output_and_fee_total_zat,
            });
        }
    }

    Ok(())
}

fn checked_sum(
    values: impl IntoIterator<Item = u64>,
    step_index: usize,
) -> Result<u128, SnapshotValidationError> {
    values.into_iter().try_fold(0_u128, |total, value| {
        total
            .checked_add(u128::from(value))
            .ok_or(SnapshotValidationError::ArithmeticOverflow { step_index })
    })
}

pub fn validate_comparison_context(
    baseline: &ProposalSnapshot,
    candidate: &ProposalSnapshot,
) -> Result<(), ComparisonContextError> {
    if baseline.schema_version != candidate.schema_version {
        return Err(ComparisonContextError::SchemaVersionMismatch {
            baseline: baseline.schema_version,
            candidate: candidate.schema_version,
        });
    }
    if baseline.network != candidate.network {
        return Err(ComparisonContextError::NetworkMismatch {
            baseline: baseline.network.clone(),
            candidate: candidate.network.clone(),
        });
    }
    Ok(())
}
