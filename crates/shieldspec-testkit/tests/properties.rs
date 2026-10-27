use std::collections::BTreeMap;

use proptest::prelude::*;
use shieldspec_core::{assess, OutputFact, OutputRole, Policy, Pool, Severity};
use shieldspec_testkit::{ProposalBuilder, StepBuilder};

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Info => 0,
        Severity::Warning => 1,
        Severity::Error => 2,
    }
}

fn finding_profile(snapshot: &shieldspec_core::ProposalSnapshot) -> BTreeMap<String, Severity> {
    assess(snapshot, &Policy::standard())
        .findings
        .into_iter()
        .map(|finding| (finding.fingerprint, finding.severity))
        .collect()
}

proptest! {
    #[test]
    fn fingerprints_survive_value_mutations_and_serde_roundtrips(
        input_value in any::<u64>(),
        output_value in any::<u64>(),
        fee in any::<u64>(),
        changed_input in any::<u64>(),
        changed_output in any::<u64>(),
        changed_fee in any::<u64>(),
    ) {
        let original = ProposalBuilder::new()
            .step(
                StepBuilder::new(0)
                    .input(Pool::Transparent, input_value)
                    .output(Pool::Orchard, OutputRole::Recipient, output_value)
                    .fee(fee),
            )
            .build();
        let mut mutated = original.clone();
        mutated.steps[0].inputs[0].value_zat = changed_input;
        mutated.steps[0].outputs[0].value_zat = changed_output;
        mutated.steps[0].fee_zat = changed_fee;
        let roundtripped = serde_json::from_str(
            &serde_json::to_string(&original).expect("serialize snapshot")
        ).expect("deserialize snapshot");

        prop_assert_eq!(finding_profile(&original), finding_profile(&mutated));
        prop_assert_eq!(finding_profile(&original), finding_profile(&roundtripped));
    }

    #[test]
    fn adding_transparent_exposure_cannot_reduce_findings_or_severity(
        shielded_inputs in prop::collection::vec(any::<u64>(), 1..8),
        shielded_outputs in prop::collection::vec(any::<u64>(), 1..8),
        existing_transparent_input in any::<bool>(),
        existing_transparent_recipient in any::<bool>(),
        transparent_value in any::<u64>(),
    ) {
        let mut step = StepBuilder::new(0);
        for value in shielded_inputs {
            step = step.input(Pool::Orchard, value);
        }
        for value in shielded_outputs {
            step = step.output(Pool::Orchard, OutputRole::Recipient, value);
        }
        if existing_transparent_input {
            step = step.input(Pool::Transparent, transparent_value);
        }
        if existing_transparent_recipient {
            step = step.output(Pool::Transparent, OutputRole::Recipient, transparent_value);
        }
        let baseline = ProposalBuilder::new().step(step).build();
        let baseline_profile = finding_profile(&baseline);

        let mut exposed = baseline.clone();
        exposed.steps[0].outputs.push(OutputFact {
            pool: Pool::Transparent,
            role: OutputRole::Change,
            value_zat: transparent_value,
        });
        let exposed_profile = finding_profile(&exposed);

        prop_assert!(exposed_profile.len() >= baseline_profile.len());
        for (fingerprint, severity) in baseline_profile {
            let exposed_severity = exposed_profile
                .get(&fingerprint)
                .expect("existing structural finding remains");
            prop_assert!(severity_rank(*exposed_severity) >= severity_rank(severity));
        }
        prop_assert!(exposed_profile.values().any(|severity| *severity == Severity::Error));
    }
}
