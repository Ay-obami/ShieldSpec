# Demo storyboard

## The setup

A wallet PR still sends the payment to an Orchard recipient. The destination-address check looks fine, but updated change selection sends the remainder to the transparent pool.

Show the two proposal shapes:

```text
baseline:  Orchard input -> Orchard recipient + Orchard change
candidate: Orchard input -> Orchard recipient + Transparent change
```

No transaction is signed or broadcast. Both inputs are local, deterministic proposal snapshots.

## The magic moment

From the repository root:

```bash
cargo run -p shieldspec-cli -- diff \
  --baseline fixtures/shielded-payment.json \
  --candidate fixtures/transparent-change.json \
  --policy standard \
  --format human
```

Point out that the recipient is shielded in both files. Then show the non-zero result: the diff includes `SS003_TRANSPARENT_CHANGE` and exits `2`, turning the hidden change regression into a testable CI failure.

Run the same scenario as a wallet developer:

```bash
cargo test --manifest-path examples/mock-wallet/Cargo.toml
```

The test asserts three things: the baseline assessment passes, the candidate comparison fails, and the regression findings contain `SS003_TRANSPARENT_CHANGE`.

## CI close

Show the GitHub Action running the same baseline and candidate with `format: sarif`. Emphasize that it uses proposal snapshots only and needs no wallet secrets.

Close with the precise claim: ShieldSpec caught a structural privacy regression before the wallet change shipped. Do not describe a passing result as proof of anonymity; it only means the supplied proposal facts triggered no configured failure.
