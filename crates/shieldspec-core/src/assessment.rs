use serde::{Deserialize, Serialize};

use crate::rules::evaluate;
use crate::validation::{snapshot_validation_finding, validate_snapshot};
use crate::{Finding, Policy, ProposalSnapshot};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Assessment {
    pub findings: Vec<Finding>,
    pub passes: bool,
}

#[must_use]
pub fn assess(snapshot: &ProposalSnapshot, policy: &Policy) -> Assessment {
    let mut findings = evaluate(snapshot, policy);
    if let Err(error) = validate_snapshot(snapshot) {
        findings.push(snapshot_validation_finding(&error, "snapshot"));
    }
    Assessment {
        passes: findings.is_empty(),
        findings,
    }
}
