use serde::{Deserialize, Serialize};

use crate::rules::evaluate;
use crate::{Finding, Policy, ProposalSnapshot};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Assessment {
    pub findings: Vec<Finding>,
    pub passes: bool,
}

#[must_use]
pub fn assess(snapshot: &ProposalSnapshot, policy: &Policy) -> Assessment {
    let findings = evaluate(snapshot, policy);
    Assessment {
        passes: findings.is_empty(),
        findings,
    }
}
