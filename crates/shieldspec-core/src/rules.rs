use std::collections::BTreeMap;

use crate::fingerprint::fingerprint;
use crate::{Finding, OutputRole, Policy, Pool, ProposalSnapshot, Severity, StepSnapshot};

pub(crate) const TRANSPARENT_RECIPIENT: &str = "SS001_TRANSPARENT_RECIPIENT";
pub(crate) const TRANSPARENT_FUNDING: &str = "SS002_TRANSPARENT_FUNDING";
pub(crate) const TRANSPARENT_CHANGE: &str = "SS003_TRANSPARENT_CHANGE";
pub(crate) const UNKNOWN_ROUTE: &str = "SS004_UNKNOWN_ROUTE";
pub(crate) const PRIVACY_BOUNDARY: &str = "SS005_PRIVACY_BOUNDARY";

pub(crate) fn evaluate(snapshot: &ProposalSnapshot, policy: &Policy) -> Vec<Finding> {
    let mut findings = Vec::new();
    for step in &snapshot.steps {
        transparent_recipients(step, policy, &mut findings);
        transparent_funding(step, policy, &mut findings);
        transparent_change(step, policy, &mut findings);
        unknown_routes(step, policy, &mut findings);
        privacy_boundaries(step, policy, &mut findings);
    }
    findings
}

fn transparent_recipients(step: &StepSnapshot, policy: &Policy, findings: &mut Vec<Finding>) {
    for (position, output) in step.outputs.iter().enumerate() {
        if output.pool == Pool::Transparent && output.role == OutputRole::Recipient {
            findings.push(output_finding(
                TRANSPARENT_RECIPIENT,
                policy.severity(Severity::Warning),
                step,
                position,
                "Recipient output uses the transparent pool",
            ));
        }
    }
}

fn transparent_funding(step: &StepSnapshot, policy: &Policy, findings: &mut Vec<Finding>) {
    for (position, input) in step.inputs.iter().enumerate() {
        if input.pool == Pool::Transparent {
            let position_text = position.to_string();
            let evidence = BTreeMap::from([
                ("input_index".to_owned(), position_text.clone()),
                ("pool".to_owned(), input.pool.name().to_owned()),
                ("value_zat".to_owned(), input.value_zat.to_string()),
            ]);
            findings.push(Finding {
                rule_id: TRANSPARENT_FUNDING.to_owned(),
                severity: policy.severity(Severity::Warning),
                step_index: step.index,
                message: "Proposal spends transparent funds".to_owned(),
                evidence,
                fingerprint: fingerprint(
                    TRANSPARENT_FUNDING,
                    step.index,
                    &["input", &position_text, input.pool.name()],
                ),
            });
        }
    }
}

fn transparent_change(step: &StepSnapshot, policy: &Policy, findings: &mut Vec<Finding>) {
    for (position, output) in step.outputs.iter().enumerate() {
        if output.pool == Pool::Transparent && output.role == OutputRole::Change {
            findings.push(output_finding(
                TRANSPARENT_CHANGE,
                policy.severity(Severity::Error),
                step,
                position,
                "Change output uses the transparent pool",
            ));
        }
    }
}

fn unknown_routes(step: &StepSnapshot, policy: &Policy, findings: &mut Vec<Finding>) {
    for (position, input) in step.inputs.iter().enumerate() {
        if input.pool == Pool::Unknown {
            let position_text = position.to_string();
            findings.push(Finding {
                rule_id: UNKNOWN_ROUTE.to_owned(),
                severity: policy.severity(Severity::Warning),
                step_index: step.index,
                message: "Input route has an unknown pool".to_owned(),
                evidence: BTreeMap::from([
                    ("input_index".to_owned(), position_text.clone()),
                    ("pool".to_owned(), input.pool.name().to_owned()),
                    ("value_zat".to_owned(), input.value_zat.to_string()),
                ]),
                fingerprint: fingerprint(
                    UNKNOWN_ROUTE,
                    step.index,
                    &["input", &position_text, input.pool.name()],
                ),
            });
        }
    }
    for (position, output) in step.outputs.iter().enumerate() {
        if output.pool == Pool::Unknown {
            findings.push(output_finding(
                UNKNOWN_ROUTE,
                policy.severity(Severity::Warning),
                step,
                position,
                "Output route has an unknown pool",
            ));
        }
    }
}

fn privacy_boundaries(step: &StepSnapshot, policy: &Policy, findings: &mut Vec<Finding>) {
    for (input_position, input) in step.inputs.iter().enumerate() {
        for (output_position, output) in step.outputs.iter().enumerate() {
            let crosses_boundary = (input.pool == Pool::Transparent && output.pool.is_shielded())
                || (input.pool.is_shielded() && output.pool == Pool::Transparent);
            if crosses_boundary {
                let input_position_text = input_position.to_string();
                let output_position_text = output_position.to_string();
                findings.push(Finding {
                    rule_id: PRIVACY_BOUNDARY.to_owned(),
                    severity: policy.severity(Severity::Warning),
                    step_index: step.index,
                    message: "Value crosses the transparent/shielded privacy boundary".to_owned(),
                    evidence: BTreeMap::from([
                        ("input_index".to_owned(), input_position_text.clone()),
                        ("input_pool".to_owned(), input.pool.name().to_owned()),
                        ("input_value_zat".to_owned(), input.value_zat.to_string()),
                        ("output_index".to_owned(), output_position_text.clone()),
                        ("output_pool".to_owned(), output.pool.name().to_owned()),
                        ("output_value_zat".to_owned(), output.value_zat.to_string()),
                    ]),
                    fingerprint: fingerprint(
                        PRIVACY_BOUNDARY,
                        step.index,
                        &[
                            "input",
                            &input_position_text,
                            input.pool.name(),
                            "output",
                            &output_position_text,
                            output.pool.name(),
                        ],
                    ),
                });
            }
        }
    }
}

fn output_finding(
    rule_id: &str,
    severity: Severity,
    step: &StepSnapshot,
    position: usize,
    message: &str,
) -> Finding {
    let output = &step.outputs[position];
    let position_text = position.to_string();
    Finding {
        rule_id: rule_id.to_owned(),
        severity,
        step_index: step.index,
        message: message.to_owned(),
        evidence: BTreeMap::from([
            ("output_index".to_owned(), position_text.clone()),
            ("pool".to_owned(), output.pool.name().to_owned()),
            ("role".to_owned(), output.role.name().to_owned()),
            ("value_zat".to_owned(), output.value_zat.to_string()),
        ]),
        fingerprint: fingerprint(
            rule_id,
            step.index,
            &[
                "output",
                &position_text,
                output.pool.name(),
                output.role.name(),
            ],
        ),
    }
}
