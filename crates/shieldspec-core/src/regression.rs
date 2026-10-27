use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::rules::evaluate;
use crate::validation::{
    comparison_context_finding, snapshot_validation_finding, validate_comparison_context,
    validate_snapshot,
};
use crate::{Finding, Policy, ProposalSnapshot, Severity};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RegressionReport {
    pub findings: Vec<Finding>,
    pub passes: bool,
}

#[must_use]
pub fn compare(
    baseline: &ProposalSnapshot,
    candidate: &ProposalSnapshot,
    policy: &Policy,
) -> RegressionReport {
    let baseline_findings = evaluate(baseline, policy);
    let candidate_findings = evaluate(candidate, policy);
    let mut findings = regression_findings(baseline_findings, candidate_findings);

    if let Err(error) = validate_snapshot(baseline) {
        findings.push(snapshot_validation_finding(&error, "baseline"));
    }
    if let Err(error) = validate_snapshot(candidate) {
        findings.push(snapshot_validation_finding(&error, "candidate"));
    }
    if let Err(error) = validate_comparison_context(baseline, candidate) {
        findings.push(comparison_context_finding(&error));
    }

    RegressionReport {
        passes: findings.is_empty(),
        findings,
    }
}

fn regression_findings(
    baseline_findings: Vec<Finding>,
    candidate_findings: Vec<Finding>,
) -> Vec<Finding> {
    let baseline_severities: HashMap<_, _> = baseline_findings
        .into_iter()
        .map(|finding| (finding.fingerprint, finding.severity))
        .collect();

    candidate_findings
        .into_iter()
        .filter(
            |finding| match baseline_severities.get(&finding.fingerprint) {
                Some(severity) => severity_worsened(*severity, finding.severity),
                None => true,
            },
        )
        .collect()
}

const fn severity_worsened(baseline: Severity, candidate: Severity) -> bool {
    severity_rank(candidate) > severity_rank(baseline)
}

const fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Info => 0,
        Severity::Warning => 1,
        Severity::Error => 2,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::regression_findings;
    use crate::{Finding, Severity};

    fn finding(severity: Severity) -> Finding {
        Finding {
            rule_id: "SS001_TRANSPARENT_RECIPIENT".to_owned(),
            severity,
            step_index: 0,
            message: "message does not identify the finding".to_owned(),
            evidence: BTreeMap::new(),
            fingerprint: "same-structural-fingerprint".to_owned(),
        }
    }

    #[test]
    fn worsened_severity_for_the_same_fingerprint_is_a_regression() {
        let regressions = regression_findings(
            vec![finding(Severity::Warning)],
            vec![finding(Severity::Error)],
        );

        assert_eq!(regressions, vec![finding(Severity::Error)]);
    }
}
