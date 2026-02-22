# Protocol Surface (Canonical Contracts)

Defines protocol-level contract fields referenced across receipts, replay, and sync/event streams.

Authority order:

1. Terminology: `docs/GLOSSARY.md`
2. Architecture intent: ADRs
3. Behavior: code

## Canonical IDs

- `session_id`
- `trajectory_hash`
- `job_hash`
- `policy_bundle_id`

## Hashing

Whenever hashes are emitted (`params_hash`, `output_hash`, `trajectory_hash`, etc.), hashing must be deterministic over canonicalized input.

Recommended encoding:

- `sha256:<hex>`

## Monetary Receipt Model

Payment receipts must use `(rail, asset_id, amount)` semantics.

Required payment fields:

- `rail`
- `asset_id`
- `payment_proof`
- amount field (`amount_msats` for msat-native rails)

`payment_proof` is a typed object:

```json
{
  "type": "lightning_preimage | cashu_proof | onchain_txid | taproot_assets_proof",
  "value": "..."
}
```

## Minimal PaymentReceipt Shape

```jsonc
{
  "rail": "lightning",
  "asset_id": "BTC_LN",
  "amount_msats": 123000,
  "payment_proof": { "type": "lightning_preimage", "value": "<hex>" },
  "session_id": "<session_id>",
  "trajectory_hash": "<trajectory_hash>",
  "policy_bundle_id": "<policy_bundle_id>",
  "job_hash": "<job_hash>"
}
```

## Related Docs

- `docs/execution/ARTIFACTS.md`
- `docs/execution/REPLAY.md`
- `docs/dse/TOOLS.md`
