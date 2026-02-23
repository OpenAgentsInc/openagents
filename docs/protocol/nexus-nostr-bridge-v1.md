# Nexus <-> Nostr Bridge (Phase-0 Minimal) v1

Status: Active  
Last updated: 2026-02-23

This document defines the **Phase-0 Bridge boundary** between:

- **Nexus**: high-throughput intra-domain fabric (registry, orchestration, streaming, coordination)
- **Nostr**: interop substrate (portable, audit-friendly events consumable outside the operator domain)
- **Bridge**: policy + translation layer controlling what crosses domains and how it is proven

This boundary is intentionally strict in Phase 0: **only provider ads and receipt pointers are mirrored to Nostr**.

## Invariants

- Authority mutations are authenticated HTTP only (`INV-02`).
- Live delivery lanes are WebSocket only (`INV-03`); no SSE is introduced as an authority source.
- Idempotency + replay semantics are preserved (`INV-07`).

The Bridge does not create authority. It mirrors already-authoritative outcomes into Nostr-verifiable form.

## Trust Zones

- Zone 0: operator-domain services (Nexus + runtime authority components)
- Zone 0.5: **account-attached devices** enrolled to a user account (authenticated and quota'd, but still sandboxed and still verified)
- Zone 1: external providers/agents (hostile-by-default; signed + rate-limited + replay-safe)

## Message Classes

Two message classes are enforced conceptually even if transport differs:

1. **Class 1: authority mutations** (money/state/rights)
   - MUST be attributable, receipted, and idempotent.
   - In Rust-era repo invariants this means: authenticated HTTP commands (`INV-02`) producing deterministic receipts.
   - May be mirrored to Nostr by the Bridge for neutral verification/portability.
2. **Class 2: ephemeral coordination** (progress, streaming, heartbeats)
   - MAY be session-authenticated (transport security) and is not mirrored to Nostr in Phase 0.

## Phase-0 Nostr Event Surface (Minimal)

### 1) Provider Ads (NIP-89 Handler Info)

Purpose:
- Make OpenAgents Compute providers discoverable outside the operator domain.

Event:
- Kind: **31990** (NIP-89 handler information)
- `["handler","compute_provider"]`
- `["d","openagents:compute_provider:<provider_id>"]` (handler identifier)
- One or more `["capability", "..."]` tags
- Optional `["price", "<msats>", "<model>", "<currency>"]`

OpenAgents Bridge-required tags:
- `["oa_schema","openagents.bridge.provider_ad.v1"]`
- `["oa_provider_id","<provider_id>"]`

Optional OpenAgents tags (Phase 1+ multi-homing sync):
- `["oa_availability","available|unavailable"]`
- `["oa_worker_status","starting|running|stopping|stopped|failed"]`
- `["oa_heartbeat_state","fresh|stale|..."]`
- `["oa_caps","<json>"]` (resource caps/policy hints)

Content:
- NIP-89 handler metadata JSON (name/description/website/icon)

### 2) Receipt Pointers (NIP-78 App Data)

Purpose:
- Publish a **portable pointer** to verification/settlement receipts without moving high-rate execution chatter onto Nostr.

Event:
- Kind: **30078** (NIP-78 application-specific data)
- `["d","openagents:receipt_ptr:<receipt_sha256>"]`

OpenAgents Bridge-required tags:
- `["oa_schema","openagents.bridge.receipt_ptr.v1"]`
- `["oa_provider_id","<provider_id>"]`
- `["oa_run_id","<uuid>"]`
- `["oa_job_hash","<job_hash>"]`
- `["oa_settlement","released|withheld|..."]`
- `["oa_receipt_sha256","<hex_sha256>"]`

Content:
- JSON payload `ReceiptPointerV1` (schema-owned by OpenAgents):
  - `provider_id`
  - `run_id`
  - `job_hash`
  - `receipt_sha256`
  - `settlement_status`
  - `receipt_url`

Notes:
- Receipt pointers are not an authority source. The authoritative receipt remains in the runtime receipt artifact.
- Wallet executor receipt canonicalization is governed by `ADR-0006`. Runtime treasury payment receipts use `openagents.treasury.payment_receipt.v1` embedded in the runtime receipt artifact (this bridge pointer is a Phase-0 minimal interop affordance).

## Phase-1+ Nostr Event Surface (Marketplace Commerce)

Phase 1 may mirror **low-rate marketplace commerce messages** to Nostr to enable
cross-domain interop (agents/providers that do not run inside a single operator
domain).

These are portable contract surfaces (RFQ/Offer/Quote/Accept/Cancel/Receipt/Refund/Dispute),
not streaming/orchestration chatter.

Canonical grammar:

- `docs/protocol/marketplace-commerce-grammar-v1.md`
- `proto/openagents/runtime/v1/commerce.proto` (`openagents.runtime.v1`)

Recommended Nostr encoding:

- Kind: **30078** (NIP-78 application data)
- `["d","openagents:commerce:<kind>:<message_id>"]`
- Tags include:
  - `oa_schema=openagents.bridge.commerce_message.v1`
  - `oa_commerce_kind=rfq|offer|quote|accept|cancel|receipt|refund|dispute`
  - linkage tags (`oa_rfq_id`, `oa_quote_id`, `oa_order_id`, etc.) when applicable

Important constraint:

- Commerce events mirrored to Nostr do not mutate authority directly; authority
  application remains authenticated HTTP (`INV-02`) and produces receipts.

## Explicit Non-Goals (Phase 0)

- No job streaming, token streams, or internal coordination mirrored to Nostr.
- No NIP-90 job request/result authority flows (planned separately).
- No reliance on Nostr to mutate Nexus authority state.

## Reference Implementation

Runtime-service Bridge helpers live at:

- `apps/runtime/src/bridge.rs`

Phase-0 vignette emits real Nostr-verifiable events to:

- `output/vignettes/phase0/<ts>/bridge_nostr_events.jsonl`

See:

- `docs/plans/active/vignette-phase0-issue-to-pr.md`
- `scripts/vignette-phase0-issue-to-pr.sh`
