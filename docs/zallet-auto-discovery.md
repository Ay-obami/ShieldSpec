# Automated rediscovery of historical Zallet #677

ShieldSpec can run its general differential fuzzer against two independently
compiled historical revisions of Zallet's actual **FullPrivacy** enforcement.
This is a regression reproduction of a **publicly documented, already fixed**
Zcash wallet-core issue: [Zallet #677](https://github.com/zcash/zallet/pull/677).
It is not a newly discovered vulnerability.

## What executes

- Affected Zallet revision: f496c9b6b2c6ebb0065b329c11bda4a5fdf73568.
- Corrected revision: e966780216b7a86b0a1a6618f2fde5b574da7171.
- The identical **test-only** Rust dynamic probe is injected as a cfg(test)
  module into each pinned upstream source checkout, without changing the
  production policy body or altering the original fixed historical witness.
- Each compiled binary constructs a real typed in-memory Zcash Proposal,
  reads its actual pool/amount facts and calls its own production FullPrivacy
  function. The JSON process adapter strictly validates these observations.
- Generated executable launchers pin the manifest and adapter SHA-256.
  The manifest pins each independently compiled binary and source revision;
  ShieldSpec hashes the launcher itself for discovery save/replay.

This is a synthetic, no-recipient proposal with **one Orchard-family input and
one shielded change output**. It exercises the real wallet-core privacy policy,
but not the complete database, wallet payment selection, signing or broadcast.
No private keys or funded wallets are needed.

## Reproduce end-to-end

Requires Python 3, Git, Rust 1.95.0, network access to fetch historical Zallet
commits, and enough disk for a real Rust wallet-core build. Run from repo root:

~~~bash
python3 scripts/build_zallet_policy_runners.py \
  --output-dir /tmp/shieldspec-zallet-auto \
  --manifest /tmp/shieldspec-zallet-auto/manifest.json \
  --cargo-target-dir /tmp/shieldspec-zallet-auto-cargo

python3 scripts/make_zallet_immutable_launchers.py \
  --manifest /tmp/shieldspec-zallet-auto/manifest.json \
  --output-dir /tmp/shieldspec-zallet-auto/launchers

cargo build --locked -p shieldspec-zcash-runner --bin shieldspec-fuzz

python3 scripts/verify_zallet_auto_discovery.py \
  --fuzzer target/debug/shieldspec-fuzz \
  --manifest /tmp/shieldspec-zallet-auto/manifest.json \
  --output-dir /tmp/shieldspec-zallet-auto/evidence
~~~

The validator demands two controls (Ironwood→Ironwood accepted on both;
Orchard→Sapling rejected on both) followed by the historical difference
(Ironwood→Sapling accepted by affected, rejected by corrected).
The general fuzzer must then automatically detect a named wallet-policy
difference, save its minimized scenario, and replay the **same** difference
from recorded SHA-linked provenance.

Output includes control JSON, campaign JSON, minimized discovery and replay.
Exit **2** signifies a genuine comparable policy difference; malformed inputs,
missing verdicts, changed executable/hash, crashes and incomplete observations
are operational errors. The Zallet privacy verdict is not inferred from
ShieldSpec's separate snapshot-policy warnings.

## CI and interpretation

The new GitHub Actions workflow compiles both actual pinned source revisions,
runs the controls, general fuzz campaign and saved replay, then uploads evidence.
The baseline is deliberately the **corrected** code and the candidate the
**historically affected** code: baseline rejects, candidate accepts.
That does not imply modern Zallet has regressed or that we found a new bug.

The original standalone [historical evidence](../evidence/historical-zallet-677/README.md)
remains valid and independent. The previous original historical source witness
and recorded logs are not modified by this integration.
