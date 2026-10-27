//! Core privacy-policy types and analysis for ShieldSpec.

mod assessment;
mod fingerprint;
mod model;
mod policy;
mod regression;
mod rules;
mod validation;

pub use assessment::{assess, Assessment};
pub use model::{
    Finding, InputFact, OutputFact, OutputRole, Pool, ProposalSnapshot, Severity, StepSnapshot,
};
pub use policy::Policy;
pub use regression::{compare, RegressionReport};
pub use validation::{
    validate_comparison_context, validate_snapshot, ComparisonContextError, SnapshotValidationError,
};
