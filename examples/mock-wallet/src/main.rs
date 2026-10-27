use shieldspec_core::{
    compare, InputFact, OutputFact, OutputRole, Policy, Pool, ProposalSnapshot, StepSnapshot,
};

fn wallet_proposals() -> (ProposalSnapshot, ProposalSnapshot) {
    let shielded_recipient = OutputFact {
        pool: Pool::Orchard,
        role: OutputRole::Recipient,
        value_zat: 100_000,
    };
    let snapshot = |change_pool| ProposalSnapshot {
        schema_version: 1,
        network: "testnet".to_owned(),
        steps: vec![StepSnapshot {
            index: 0,
            inputs: vec![InputFact {
                pool: Pool::Orchard,
                value_zat: 110_000,
                origin: Default::default(),
            }],
            outputs: vec![
                shielded_recipient.clone(),
                OutputFact {
                    pool: change_pool,
                    role: OutputRole::Change,
                    value_zat: 9_000,
                },
            ],
            fee_zat: 1_000,
        }],
    };

    (snapshot(Pool::Orchard), snapshot(Pool::Transparent))
}

fn main() {
    let (baseline, candidate) = wallet_proposals();
    let report = compare(&baseline, &candidate, &Policy::standard());

    if report.passes {
        println!("privacy regression check passed");
    } else {
        for finding in &report.findings {
            println!("{}: {}", finding.rule_id, finding.message);
        }
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use shieldspec_core::{assess, compare, Policy, Pool};
    use shieldspec_testkit::{ProposalBuilder, StepBuilder};

    use super::wallet_proposals;

    #[test]
    fn shielded_recipient_with_transparent_change_is_a_regression() {
        // These public builder types are part of the downstream integration surface.
        let _builder_types = (
            std::any::type_name::<ProposalBuilder>(),
            std::any::type_name::<StepBuilder>(),
        );
        let (baseline, candidate) = wallet_proposals();
        let policy = Policy::standard();

        assert!(assess(&baseline, &policy).passes);
        assert_eq!(candidate.steps[0].outputs[0].pool, Pool::Orchard);
        let report = compare(&baseline, &candidate, &policy);

        assert!(!report.passes);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.rule_id == "SS003_TRANSPARENT_CHANGE"));
    }
}
