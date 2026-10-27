use std::collections::BTreeMap;

use crate::fingerprint::fingerprint;
use crate::{Finding, OutputRole, Policy, Pool, ProposalSnapshot, Severity, StepSnapshot};

pub(crate) const TRANSPARENT_RECIPIENT: &str = "SS001_TRANSPARENT_RECIPIENT";
pub(crate) const TRANSPARENT_FUNDING: &str = "SS002_TRANSPARENT_FUNDING";
pub(crate) const TRANSPARENT_CHANGE: &str = "SS003_TRANSPARENT_CHANGE";
pub(crate) const UNKNOWN_ROUTE: &str = "SS004_UNKNOWN_ROUTE";
pub(crate) const PRIVACY_BOUNDARY: &str = "SS005_PRIVACY_BOUNDARY";
pub(crate) const CROSS_SHIELDED_POOL_VALUE_BALANCE: &str =
    "SS006_CROSS_SHIELDED_POOL_VALUE_BALANCE";

pub(crate) fn evaluate(snapshot: &ProposalSnapshot, policy: &Policy) -> Vec<Finding> {
    let mut findings = Vec::new();
    for step in &snapshot.steps {
        transparent_recipients(step, policy, &mut findings);
        transparent_funding(step, policy, &mut findings);
        transparent_change(step, policy, &mut findings);
        unknown_routes(step, policy, &mut findings);
    }
    privacy_boundaries(snapshot, policy, &mut findings);
    cross_shielded_pool_value_balances(snapshot, policy, &mut findings);
    findings
}

fn transparent_recipients(step: &StepSnapshot, policy: &Policy, findings: &mut Vec<Finding>) {
    for (position, output) in step.outputs.iter().enumerate() {
        if output.pool == Pool::Transparent && output.role == OutputRole::Recipient {
            let occurrence = output_occurrence(step, position);
            findings.push(output_finding(
                TRANSPARENT_RECIPIENT,
                policy.severity(Severity::Warning),
                step,
                position,
                occurrence,
                "Recipient output uses the transparent pool",
            ));
        }
    }
}

fn transparent_funding(step: &StepSnapshot, policy: &Policy, findings: &mut Vec<Finding>) {
    for (position, input) in step.inputs.iter().enumerate() {
        if input.pool == Pool::Transparent && input.origin.is_wallet() {
            let position_text = position.to_string();
            let occurrence_text = input_occurrence(step, position).to_string();
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
                    &["input", input.pool.name(), &occurrence_text],
                ),
            });
        }
    }
}

fn transparent_change(step: &StepSnapshot, policy: &Policy, findings: &mut Vec<Finding>) {
    for (position, output) in step.outputs.iter().enumerate() {
        if output.pool == Pool::Transparent && output.role == OutputRole::Change {
            let occurrence = output_occurrence(step, position);
            findings.push(output_finding(
                TRANSPARENT_CHANGE,
                policy.severity(Severity::Error),
                step,
                position,
                occurrence,
                "Change output uses the transparent pool",
            ));
        }
    }
}

fn unknown_routes(step: &StepSnapshot, policy: &Policy, findings: &mut Vec<Finding>) {
    for (position, input) in step.inputs.iter().enumerate() {
        if input.pool == Pool::Unknown {
            let position_text = position.to_string();
            let occurrence_text = input_occurrence(step, position).to_string();
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
                    &["input", input.pool.name(), &occurrence_text],
                ),
            });
        }
    }
    for (position, output) in step.outputs.iter().enumerate() {
        if output.pool == Pool::Unknown {
            let occurrence = output_occurrence(step, position);
            findings.push(output_finding(
                UNKNOWN_ROUTE,
                policy.severity(Severity::Warning),
                step,
                position,
                occurrence,
                "Output route has an unknown pool",
            ));
        }
    }
}

fn privacy_boundaries(snapshot: &ProposalSnapshot, policy: &Policy, findings: &mut Vec<Finding>) {
    let totals = FlowTotals {
        transparent_inputs: proposal_input_total(snapshot, PoolKind::Transparent),
        transparent_outputs: proposal_output_total(snapshot, PoolKind::Transparent),
        shielded_inputs: proposal_input_total(snapshot, PoolKind::Shielded),
        shielded_outputs: proposal_output_total(snapshot, PoolKind::Shielded),
    };

    if totals.transparent_outputs > totals.transparent_inputs {
        let step_index = boundary_step(snapshot, "shielded_to_transparent");
        boundary_finding(
            step_index,
            policy,
            "shielded_to_transparent",
            totals.transparent_outputs - totals.transparent_inputs,
            totals,
            findings,
        );
    }
    if totals.shielded_outputs > totals.shielded_inputs {
        let step_index = boundary_step(snapshot, "transparent_to_shielded");
        boundary_finding(
            step_index,
            policy,
            "transparent_to_shielded",
            totals.shielded_outputs - totals.shielded_inputs,
            totals,
            findings,
        );
    }
}

fn cross_shielded_pool_value_balances(
    snapshot: &ProposalSnapshot,
    policy: &Policy,
    findings: &mut Vec<Finding>,
) {
    // A shielded pool's public value balance is observable even though
    // individual shielded notes are not. A fee-only balance in one pool
    // must not trigger this cross-pool observation rule.
    const SHIELDED_POOLS: [Pool; 3] = [Pool::Sapling, Pool::Orchard, Pool::Ironwood];

    for step in &snapshot.steps {
        let mut outflows = Vec::new();
        let mut inflows = Vec::new();
        for pool in SHIELDED_POOLS {
            let input_zat: u128 = step
                .inputs
                .iter()
                .filter(|input| input.pool == pool)
                .map(|input| u128::from(input.value_zat))
                .sum();
            let output_zat: u128 = step
                .outputs
                .iter()
                .filter(|output| output.pool == pool && output.role != OutputRole::Ephemeral)
                .map(|output| u128::from(output.value_zat))
                .sum();
            if input_zat > output_zat {
                outflows.push((pool.name(), input_zat - output_zat));
            } else if output_zat > input_zat {
                inflows.push((pool.name(), output_zat - input_zat));
            }
        }

        if outflows.is_empty() || inflows.is_empty() {
            continue;
        }

        let source_pools = outflows
            .iter()
            .map(|(pool, _)| *pool)
            .collect::<Vec<_>>()
            .join(",");
        let destination_pools = inflows
            .iter()
            .map(|(pool, _)| *pool)
            .collect::<Vec<_>>()
            .join(",");
        let source_outflow: u128 = outflows.iter().map(|(_, value)| value).sum();
        let destination_inflow: u128 = inflows.iter().map(|(_, value)| value).sum();

        findings.push(Finding {
            rule_id: CROSS_SHIELDED_POOL_VALUE_BALANCE.to_owned(),
            severity: policy.severity(Severity::Warning),
            step_index: step.index,
            message: "Shielded-pool transition exposes public pool value balances".to_owned(),
            evidence: BTreeMap::from([
                ("source_pool".to_owned(), source_pools.clone()),
                ("destination_pool".to_owned(), destination_pools.clone()),
                (
                    "crossing_value_zat".to_owned(),
                    source_outflow.min(destination_inflow).to_string(),
                ),
                ("source_outflow_zat".to_owned(), source_outflow.to_string()),
                (
                    "destination_inflow_zat".to_owned(),
                    destination_inflow.to_string(),
                ),
            ]),
            fingerprint: fingerprint(
                CROSS_SHIELDED_POOL_VALUE_BALANCE,
                step.index,
                &[&source_pools, &destination_pools],
            ),
        });
    }
}

#[derive(Clone, Copy)]
enum PoolKind {
    Transparent,
    Shielded,
}

#[derive(Clone, Copy)]
struct FlowTotals {
    transparent_inputs: u128,
    transparent_outputs: u128,
    shielded_inputs: u128,
    shielded_outputs: u128,
}

fn proposal_input_total(snapshot: &ProposalSnapshot, kind: PoolKind) -> u128 {
    snapshot
        .steps
        .iter()
        .flat_map(|step| step.inputs.iter())
        .filter(|input| input.origin.is_wallet())
        .filter(|input| pool_matches(input.pool, kind))
        .map(|input| u128::from(input.value_zat))
        .sum()
}

fn proposal_output_total(snapshot: &ProposalSnapshot, kind: PoolKind) -> u128 {
    snapshot
        .steps
        .iter()
        .flat_map(|step| step.outputs.iter())
        .filter(|output| output.role != OutputRole::Ephemeral)
        .filter(|output| pool_matches(output.pool, kind))
        .map(|output| u128::from(output.value_zat))
        .sum()
}

const fn pool_matches(pool: Pool, kind: PoolKind) -> bool {
    match kind {
        PoolKind::Transparent => matches!(pool, Pool::Transparent),
        PoolKind::Shielded => pool.is_shielded(),
    }
}

fn boundary_step(snapshot: &ProposalSnapshot, direction: &str) -> usize {
    snapshot
        .steps
        .iter()
        .find(|step| match direction {
            "shielded_to_transparent" => step.outputs.iter().any(|output| {
                output.pool == Pool::Transparent && output.role != OutputRole::Ephemeral
            }),
            "transparent_to_shielded" => {
                step.outputs.iter().any(|output| output.pool.is_shielded())
            }
            _ => false,
        })
        .map_or(0, |step| step.index)
}

fn boundary_finding(
    step_index: usize,
    policy: &Policy,
    direction: &str,
    crossing_value_zat: u128,
    totals: FlowTotals,
    findings: &mut Vec<Finding>,
) {
    findings.push(Finding {
        rule_id: PRIVACY_BOUNDARY.to_owned(),
        severity: policy.severity(Severity::Warning),
        step_index,
        message: format!("Value crosses the privacy boundary {direction}"),
        evidence: BTreeMap::from([
            ("direction".to_owned(), direction.to_owned()),
            (
                "crossing_value_zat".to_owned(),
                crossing_value_zat.to_string(),
            ),
            (
                "transparent_input_zat".to_owned(),
                totals.transparent_inputs.to_string(),
            ),
            (
                "transparent_output_zat".to_owned(),
                totals.transparent_outputs.to_string(),
            ),
            (
                "shielded_input_zat".to_owned(),
                totals.shielded_inputs.to_string(),
            ),
            (
                "shielded_output_zat".to_owned(),
                totals.shielded_outputs.to_string(),
            ),
        ]),
        fingerprint: fingerprint(PRIVACY_BOUNDARY, step_index, &[direction]),
    });
}

fn output_finding(
    rule_id: &str,
    severity: Severity,
    step: &StepSnapshot,
    position: usize,
    occurrence: usize,
    message: &str,
) -> Finding {
    let output = &step.outputs[position];
    let position_text = position.to_string();
    let occurrence_text = occurrence.to_string();
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
                output.pool.name(),
                output.role.name(),
                &occurrence_text,
            ],
        ),
    }
}

fn input_occurrence(step: &StepSnapshot, position: usize) -> usize {
    let pool = step.inputs[position].pool;
    step.inputs[..position]
        .iter()
        .filter(|input| input.pool == pool)
        .count()
}

fn output_occurrence(step: &StepSnapshot, position: usize) -> usize {
    let output = &step.outputs[position];
    step.outputs[..position]
        .iter()
        .filter(|prior| prior.pool == output.pool && prior.role == output.role)
        .count()
}
