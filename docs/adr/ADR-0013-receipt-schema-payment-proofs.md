# ADR-0013: Receipt Schema, Payment Proof Types, and Rail/AssetId Semantics

## Status

**Accepted**

## Date

2026-01-13

## Context

Receipts bind spending to work (job_hash, trajectory_hash, policy_bundle_id). Several terms are easy to misuse:
- "proof" is overloaded (Cashu proof vs generic proof),
- "USD" without issuer/rail is ambiguous,
- payment evidence differs by rail (LN preimage vs on-chain txid vs Cashu Proof).

We need canonical receipt semantics to make spending auditable and machine-verifiable.

## Decision

**Receipts MUST use canonical payment proof types and MUST represent value via (rail, asset_id) as defined in GLOSSARY.md.**

### Canonical owner

- Canonical terminology: [GLOSSARY.md](../GLOSSARY.md) (`Rail`, `AssetId`, `Cashu Proof`, `Reconciliation`)
- Canonical protocol-level receipt fields: [docs/protocol/PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md)
- Session receipt schema (Verified Patch Bundle): [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md)

### Normative rules

1. Receipt MUST include:
   - `session_id`
   - `trajectory_hash`
   - `policy_bundle_id`
   - `job_hash` (if any external job/payment occurred)
   - `rail`
   - `asset_id`
   - `amount_msats` (or a rail-appropriate amount field if non-msats; if so, must be unambiguous)
   - `payment_proof` (typed)

2. payment_proof is a typed object:
   ```json
   {
     "type": "lightning_preimage | cashu_proof | onchain_txid | taproot_assets_proof",
     "value": "..."
   }
   ```
   Valid `type` values:
   - `lightning_preimage`
   - `cashu_proof` (**use term "Cashu Proof"** in docs)
   - `onchain_txid`
   - `taproot_assets_proof` (if/when used)

3. `asset_id` MUST be fully-qualified and MUST NOT be a bare ticker.
   - Example: `BTC_LN` vs `USD_CASHU(<mint_url>)`
   - "USD" alone is invalid in receipts.

4. Docs MUST NOT call Cashu Proofs "proofs" without the qualifier "Cashu Proof".

### Relationship: session receipt vs payment receipt

- `RECEIPT.json` (Verified Patch Bundle) is the **session receipt**.
- When a session triggers payments, it may include one or more **payment receipt entries** inside `RECEIPT.json` (shape per PROTOCOL_SURFACE minimal fields).
- Any standalone network receipt format must embed/point to the same canonical fields.

## Scope

What this ADR covers:
- payment_proof typing
- rail/asset_id semantics
- naming rules around Cashu Proof
- relationship between session receipts and job/payment receipts

What this ADR does NOT cover:
- treasury quote state machine + reconciliation (separate ADR)
- FX/exchange semantics (separate ADR)
- exact file layout (ADR-0008)

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Identifier | `policy_bundle_id` is canonical |
| Asset identity | Must be `AssetId` (rail-qualified) |
| Cashu naming | Use "Cashu Proof" term |
| Proof typing | payment_proof uses `{type, value}` format per PROTOCOL_SURFACE.md |

Backward compatibility:
- New proof types may be added, but existing tags remain stable.
- Removing/renaming proof types requires superseding ADR + migration.

## Consequences

**Positive:**
- Receipts become unambiguous across rails/currencies
- Enables budget enforcement and auditing without guessing
- Stops terminology drift ("proof" collisions)

**Negative:**
- Slightly more verbose receipt structures

**Neutral:**
- Amount normalization may evolve (msats vs sats) but must remain unambiguous

## Alternatives Considered

1. **Bare currency tickers (USD/BTC)** — rejected (ambiguous issuer/rail risk).
2. **Unstructured "payment_proof: string"** — rejected (not machine-verifiable).
3. **Receipt per rail with different schemas** — rejected (hard to audit uniformly).

## References

- [GLOSSARY.md](../GLOSSARY.md) — Rail / AssetId / Cashu Proof
- [docs/protocol/PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) — receipt field set
- [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md) — session receipt schema
