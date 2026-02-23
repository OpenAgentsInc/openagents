# Marketplace Commerce Grammar v1

Status: Active  
Last updated: 2026-02-23

This document defines the **portable commerce grammar** used for marketplace lanes
(OpenAgents Compute now; skills/data later).

The goal is **agent legibility**: machine-readable, comparable quotes and receipts,
with explicit fee surfaces and deterministic linkages.

## Authority

Canonical wire authority lives in proto:

- `proto/openagents/runtime/v1/commerce.proto` (`openagents.runtime.v1`)

JSON encodings used for interop substrates (e.g. Nostr) are **derived views only**
(see `ADR-0002`).

## Invariants

- Authority mutations are authenticated HTTP only (`INV-02`).
- Live delivery lanes are WebSocket only (`INV-03`).
- Idempotency + replay semantics are preserved (`INV-07`).

## Message Kinds (v1)

The v1 grammar defines the following low-rate commerce messages:

- RFQ (request for quote)
- Offer
- Quote (all-in)
- Accept
- Cancel
- Receipt
- Refund
- Dispute

These messages are designed to be mirrored outside an operator domain for
portability and audit, without moving orchestration/streaming traffic out of Nexus.

## Quote Legibility Requirements

An all-in quote must be machine-comparable:

- Provider price and explicit fee components are surfaced separately.
- `total_price` is the all-in comparable value.
- `valid_until` makes quote windows binding and testable.

## Deterministic Linkages (v1)

The grammar includes fields intended for deterministic linkages:

- `objective_hash` (job/objective identity)
- `run_id` (authoritative runtime execution reference for compute)
- `*_receipt_sha256` + `*_receipt_url` pointers (verification + treasury)
- `canonical_sha256` (optional until canonicalization pipeline is enforced)

Canonical hashing/signature rules are implemented separately:

- `OA-ECON-022` / `OA-ECON-023`

## Nostr Mapping (Bridge / Interop)

Commerce messages MAY be mirrored to Nostr for interop/audit. This is a policy
choice at the Bridge boundary; it does not create authority.

Recommended encoding:

- NIP-78 App Data event (`kind=30078`)
- `d` tag identifier: `openagents:commerce:<kind>:<message_id>`
- Required tags:
  - `["oa_schema","openagents.bridge.commerce_message.v1"]`
  - `["oa_commerce_kind","rfq|offer|quote|accept|cancel|receipt|refund|dispute"]`
  - `["oa_marketplace_id","..."]`
  - `["oa_actor_id","..."]`
  - `["oa_body_sha256","<sha256(content)>"]`
- Optional linkage tags (present when applicable):
  - `oa_rfq_id`, `oa_offer_id`, `oa_quote_id`, `oa_order_id`, `oa_receipt_id`
  - `oa_objective_hash`, `oa_run_id`
- Content:
  - JSON payload derived from the proto message fields for the relevant kind.

Explicit non-goal:

- High-rate coordination/streaming traffic (logs, token streams, heartbeats) is
  not mirrored to Nostr.

## Reference Implementation

- `apps/runtime/src/bridge.rs`

