use std::{collections::BTreeSet, convert::Infallible, process};

use zcash_client_backend::{
    data_api::{
        testing::{
            AddressType,
            orchard::OrchardPoolTester,
            pool::{ShieldedPoolTester, dsl::TestDsl},
            sapling::SaplingPoolTester,
        },
        wallet::ConfirmationsPolicy,
        Account as _,
    },
    fees::StandardFeeRule,
};
use zcash_client_sqlite::testing::{BlockCache, db::TestDbFactory};
use zcash_keys::address::{Address, UnifiedAddress};
use zcash_protocol::{ShieldedPool, value::Zatoshis};

#[derive(Clone, Copy, Debug)]
enum RecipientKind {
    SaplingOnly,
    DualShielded,
}

#[derive(Debug, Eq, PartialEq)]
struct Shape {
    input_pools: BTreeSet<String>,
    input_count: usize,
    payment_pool: String,
    change_pools: Vec<String>,
    fee_zat: u64,
}

fn split(total: u64, count: usize) -> Vec<u64> {
    let base = total / count as u64;
    let rem = total % count as u64;
    (0..count)
        .map(|i| base + u64::from((i as u64) < rem))
        .collect()
}

fn recipient(kind: RecipientKind, tag: u8) -> Address {
    let sapling = match SaplingPoolTester::sk_default_address(&SaplingPoolTester::sk(&[tag; 32])) {
        Address::Sapling(addr) => *addr,
        other => panic!("expected Sapling address, got {other:?}"),
    };

    match kind {
        RecipientKind::SaplingOnly => Address::Sapling(Box::new(sapling)),
        RecipientKind::DualShielded => {
            let orchard = match OrchardPoolTester::sk_default_address(&OrchardPoolTester::sk(&[
                tag.wrapping_add(1);
                32
            ])) {
                Address::Unified(ua) => ua.orchard().copied().expect("Orchard receiver"),
                other => panic!("expected Orchard unified address, got {other:?}"),
            };

            UnifiedAddress::from_receivers(Some(orchard), Some(sapling), None, None, None)
                .expect("valid dual-shielded UA")
                .into()
        }
    }
}

fn run_case(
    sapling_total: u64,
    sapling_notes: usize,
    orchard_total: Option<u64>,
    orchard_notes: usize,
    payment: u64,
    kind: RecipientKind,
    tag: u8,
) -> Result<Shape, String> {
    let mut st =
        TestDsl::with_sapling_birthday_account(TestDbFactory::default(), BlockCache::new())
            .build::<SaplingPoolTester>();

    let account = st.test_account().cloned().expect("test account");
    let sapling_fvk = SaplingPoolTester::test_account_fvk(&st);
    let orchard_fvk = OrchardPoolTester::test_account_fvk(&st);

    for value in split(sapling_total, sapling_notes) {
        st.generate_next_block(
            &sapling_fvk,
            AddressType::DefaultExternal,
            Zatoshis::const_from_u64(value),
        );
    }

    match orchard_total {
        Some(total) => {
            for value in split(total, orchard_notes) {
                st.generate_next_block(
                    &orchard_fvk,
                    AddressType::DefaultExternal,
                    Zatoshis::const_from_u64(value),
                );
            }
            st.scan_cached_blocks(account.birthday().height(), sapling_notes + orchard_notes);
        }
        None => {
            st.scan_cached_blocks(account.birthday().height(), sapling_notes);
            // Keep the baseline at the same chain height as the augmented-wallet case.
            st.add_empty_blocks(orchard_notes);
        }
    }

    let to = recipient(kind, tag);
    let proposal = st
        .propose_standard_transfer::<Infallible>(
            account.id(),
            StandardFeeRule::Zip317,
            ConfirmationsPolicy::MIN,
            &to,
            Zatoshis::const_from_u64(payment),
            None,
            None,
            ShieldedPool::Sapling,
        )
        .map_err(|e| format!("{e:?}"))?;

    if proposal.steps().len() != 1 {
        return Err(format!("unexpected {}-step proposal", proposal.steps().len()));
    }

    let step = &proposal.steps().head;
    let input_notes = step
        .shielded_inputs()
        .ok_or_else(|| "proposal has no shielded inputs".to_owned())?
        .notes();

    let input_pools = input_notes
        .iter()
        .map(|n| format!("{:?}", n.note().pool()))
        .collect::<BTreeSet<_>>();

    let payment_pool = step
        .payment_pools()
        .get(&0)
        .map(|p| format!("{p:?}"))
        .ok_or_else(|| "payment 0 has no selected pool".to_owned())?;

    let change_pools = step
        .balance()
        .proposed_change()
        .iter()
        .map(|c| format!("{:?}", c.output_pool()))
        .collect::<Vec<_>>();

    Ok(Shape {
        input_pools,
        input_count: input_notes.len(),
        payment_pool,
        change_pools,
        fee_zat: step.balance().fee_required().into_u64(),
    })
}

fn is_sapling_only(shape: &Shape) -> bool {
    shape.input_pools == BTreeSet::from(["Sapling".to_owned()])
        && shape.payment_pool.contains("Sapling")
        && shape
            .change_pools
            .iter()
            .all(|pool| pool.contains("Sapling"))
}

fn main() {
    let payments = [47_000_u64, 100_000];
    let margins = [20_000_u64, 50_000];
    let sapling_note_counts = [1_usize, 3];
    let orchard_totals = [1_000_u64, 20_000, 100_000, 1_000_000];
    let orchard_note_counts = [1_usize, 2];
    let recipients = [RecipientKind::SaplingOnly, RecipientKind::DualShielded];

    let mut tested = 0_usize;
    let mut baseline_safe = 0_usize;
    let mut baseline_non_sapling = 0_usize;
    let mut operational = 0_usize;
    let mut candidates = Vec::new();

    for kind in recipients {
        for payment in payments {
            for margin in margins {
                let sapling_total = payment + margin;
                for sapling_notes in sapling_note_counts {
                    for orchard_total in orchard_totals {
                        for orchard_notes in orchard_note_counts {
                            tested += 1;
                            let tag = ((tested * 37) % 251 + 1) as u8;

                            let baseline = run_case(
                                sapling_total,
                                sapling_notes,
                                None,
                                orchard_notes,
                                payment,
                                kind,
                                tag,
                            );
                            let augmented = run_case(
                                sapling_total,
                                sapling_notes,
                                Some(orchard_total),
                                orchard_notes,
                                payment,
                                kind,
                                tag,
                            );

                            let (baseline, augmented) = match (baseline, augmented) {
                                (Ok(b), Ok(a)) => (b, a),
                                (b, a) => {
                                    operational += 1;
                                    println!(
                                        "OPERATIONAL case={tested} kind={kind:?} payment={payment} margin={margin} sap_notes={sapling_notes} orchard_total={orchard_total} orchard_notes={orchard_notes} baseline={b:?} augmented={a:?}"
                                    );
                                    continue;
                                }
                            };

                            if !is_sapling_only(&baseline) {
                                baseline_non_sapling += 1;
                                println!(
                                    "BASELINE_ROUTE case={tested} kind={kind:?} payment={payment} margin={margin} sap_notes={sapling_notes} shape={baseline:?}"
                                );
                                continue;
                            }

                            baseline_safe += 1;
                            if !is_sapling_only(&augmented) {
                                candidates.push(format!(
                                    "CANDIDATE case={tested} kind={kind:?} payment={payment} margin={margin} sap_total={sapling_total} sap_notes={sapling_notes} orchard_total={orchard_total} orchard_notes={orchard_notes} baseline={baseline:?} augmented={augmented:?}"
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    println!(
        "SUMMARY tested={tested} baseline_safe={baseline_safe} baseline_non_sapling={baseline_non_sapling} operational={operational} candidates={}",
        candidates.len()
    );

    for candidate in &candidates {
        println!("{candidate}");
    }

    if !candidates.is_empty() {
        process::exit(2);
    }
}
