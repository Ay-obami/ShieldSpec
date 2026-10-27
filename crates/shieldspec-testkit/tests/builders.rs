use shieldspec_core::{OutputRole, Pool};
use shieldspec_testkit::{ProposalBuilder, StepBuilder};

#[test]
fn builders_create_a_test_network_snapshot_with_chained_facts() {
    let snapshot = ProposalBuilder::new()
        .step(
            StepBuilder::new(3)
                .input(Pool::Sapling, 70_000)
                .output(Pool::Orchard, OutputRole::Recipient, 60_000)
                .fee(10_000),
        )
        .build();

    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.network, "test");
    assert_eq!(snapshot.steps.len(), 1);
    assert_eq!(snapshot.steps[0].index, 3);
    assert_eq!(snapshot.steps[0].inputs[0].pool, Pool::Sapling);
    assert_eq!(snapshot.steps[0].inputs[0].value_zat, 70_000);
    assert_eq!(snapshot.steps[0].outputs[0].pool, Pool::Orchard);
    assert_eq!(snapshot.steps[0].outputs[0].role, OutputRole::Recipient);
    assert_eq!(snapshot.steps[0].outputs[0].value_zat, 60_000);
    assert_eq!(snapshot.steps[0].fee_zat, 10_000);
}

#[test]
fn proposal_builder_supports_an_explicit_network() {
    let snapshot = ProposalBuilder::new().network("regtest").build();

    assert_eq!(snapshot.network, "regtest");
}
