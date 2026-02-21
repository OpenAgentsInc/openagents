# Protocol Surface (Canonical Contracts)

This document defines the **canonical protocol-level contracts** that are referenced across ADRs, receipts, and replay artifacts.

Authority:
- Terminology: `docs/GLOSSARY.md` wins.
- Behavior/implementation: code wins.
- Architecture intent: ADRs win.

## Identifiers

These identifiers appear across artifacts and telemetry.

- `session_id`
  Stable identifier for an agent run/session. Format is implementation-defined (UUID recommended).
- `trajectory_hash`
  Content hash that binds a session to its recorded trajectory/replay.
- `job_hash`
  Content hash that binds spending or external execution to an outcome/job definition.
- `policy_bundle_id`
  Identifier for the policy bundle used for the run (see `docs/plans/archived/adr-legacy-2026-02-21/ADR-0015-policy-bundles.md`).

## Hashes

Whenever a hash is emitted, it MUST be computed deterministically over canonicalized inputs (see `docs/plans/archived/adr-legacy-2026-02-21/ADR-0006-deterministic-hashing.md`).

Recommended representation:
- `sha256:<hex>` (or bare hex if the algorithm is implied by the containing schema).

## Monetary Representation

Receipts MUST represent value via `(rail, asset_id)` and MUST NOT use bare currency tickers.

Canonical terms:
- `Rail` and `AssetId` are defined in `docs/GLOSSARY.md`.

### Required Fields

- `rail` (string)
  Example: `"lightning"`, `"cashu"`, `"onchain"`.
- `asset_id` (string)
  Example: `"BTC_LN"`, `"USD_CASHU(<mint_url>)"`.
- `amount_msats` (integer)
  Amount in millisatoshis when the rail supports msats. If a rail does not naturally support msats, the receipt MUST use an unambiguous amount field and document it in the relevant artifact schema.

### payment_proof

`payment_proof` is a typed object:

```json
{
  "type": "lightning_preimage | cashu_proof | onchain_txid | taproot_assets_proof",
  "value": "..."
}
```

Normative notes:
- Use the term **Cashu Proof** (not generic "proof") for `type = "cashu_proof"`.
- New proof types may be added; existing types must remain stable (see `docs/plans/archived/adr-legacy-2026-02-21/ADR-0013-receipt-schema-payment-proofs.md`).

## Receipt Field Set (Protocol-Level)

This section defines the minimal **protocol-level** payment receipt field set. Session receipts (Verified Patch Bundle) may embed one or more payment entries using this shape.

### PaymentReceipt (minimal)

```jsonc
{
  "rail": "lightning",
  "asset_id": "BTC_LN",
  "amount_msats": 123000,
  "payment_proof": { "type": "lightning_preimage", "value": "<hex>" },

  // linkage / audit (required when available)
  "session_id": "<session_id>",
  "trajectory_hash": "<trajectory_hash>",
  "policy_bundle_id": "<policy_bundle_id>",
  "job_hash": "<job_hash>"
}
```

See also:
- `docs/execution/ARTIFACTS.md` (session receipt schema: `RECEIPT.json`)
- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0013-receipt-schema-payment-proofs.md` (normative rules)

