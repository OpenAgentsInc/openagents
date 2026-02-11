# EP212 Demo Plan: Lightning Labs-Centric L402 Buying on openagents.com

Status: Draft
Date: 2026-02-11
Owner: OpenAgents

## 1. Demo Goal

Show, inside `openagents.com`, that Autopilot can consume paid endpoints using Lightning Labs L402 flows with clear budget guardrails and receipts.

Narrative continuation from EP211:

- EP211: "ask for capabilities, they get added quickly"
- EP212: "we added paid API consumption over Lightning"

## 2. Hard Requirements for EP212

1. Effect-first architecture only.
2. New reusable package: `packages/lightning-effect` (usable by other projects).
3. Demo runs through `openagents.com` chat UX (not a standalone CLI-only demo).
4. Consume at least two paid endpoints.
5. Show one successful payment and one policy guardrail event.
6. Show receipts/proof reference in product UI/logs.

## 3. Scope for This Episode (and only this episode)

## In scope

- Buyer-side L402 flow (402 challenge -> pay -> retry -> response).
- Minimal seller-side setup to provide paid endpoints.
- Chat-visible payment state in `openagents.com`.
- Budget cap + allowlist guardrails.
- `lightning-effect` package scaffold with real working core.

## Out of scope

- Full wallet dashboard.
- Full seller marketplace.
- Mobile/desktop full parity.
- Production-grade multi-tenant billing/reconciliation.

## 4. Demo User Story

User in Autopilot chat on `openagents.com`:

1. "Fetch premium signal feed for BTC and summarize it. Max 100 sats."
2. Autopilot shows spend intent and asks for approval.
3. User approves.
4. Autopilot pays L402 endpoint and returns premium data summary.
5. User asks same query again; Autopilot reuses cached L402 credential (no second payment).
6. User asks expensive endpoint with low cap; Autopilot blocks with policy message.

## 5. Architecture (Effect-First)

## 5.1 New package: `packages/lightning-effect`

Public modules (publishable API):

1. `contracts`
   - Effect `Schema` for:
   - `L402Challenge`
   - `L402Credential`
   - `InvoicePaymentRequest`
   - `InvoicePaymentResult` (`paymentId`, `amountMsats`, `preimageHex` required)
   - `L402FetchRequest` / `L402FetchResult`
   - `SpendPolicy`
2. `errors`
   - tagged Effect errors (`ChallengeParseError`, `BudgetExceededError`, `PaymentFailedError`, etc.)
3. `services`
   - `InvoicePayer`
   - `L402Client`
   - `CredentialCache`
   - `SpendPolicyService`
4. `l402`
   - challenge parsing + auth header construction + retry logic
5. `layers`
   - Live + Test layers
6. `adapters`
   - `invoicePayerLnd` (real)
   - `invoicePayerDemo` (deterministic test/demo)

Design constraint:

- Keep conventions aligned with `~/code/nostr-effect` (Layer composition, naming, tagged errors, tests).

## 5.2 openagents.com integration points

1. `apps/web/src/effect/lightning.ts`
   - Uses `lightning-effect` services.
2. Worker endpoint:
   - `POST /api/lightning/l402/fetch`
3. Autopilot path:
   - Add one tool contract (`lightning_l402_fetch`) and route it to the endpoint.
4. UI surface:
   - Chat part/status lines for:
   - `payment.intent`
   - `payment.sent`
   - `payment.cached`
   - `payment.blocked`

## 5.3 Seller endpoint setup for demo

Run a tiny seller service with two paid routes behind Aperture:

1. `GET /premium/signal-feed?asset=BTC`
   - fixed price (example: 50 sats)
2. `GET /premium/deep-report?asset=BTC&window=30d`
   - higher or dynamic price (example: 250 sats)

These only need deterministic JSON payloads for the demo.

## 6. Endpoint Consumption Plan (Demo-Focused)

## Endpoint A: "signal-feed" (happy path)

- Cap: 100 sats
- Price: 50 sats
- Expected: payment approved + success + summary

## Endpoint A repeat call (cache path)

- Same domain + credential scope
- Expected: cached L402 credential reuse; no new payment

## Endpoint B: "deep-report" (policy deny path)

- Cap: 100 sats
- Price: 250 sats
- Expected: blocked before payment with clear explanation

## 7. Implementation Sequence (Tight)

## Phase 0: Scaffold `lightning-effect` (Day 1)

1. Run Effect guidance preflight:
   - `effect-solutions list`
   - `effect-solutions show quick-start basics services-and-layers error-handling testing`
2. Create package skeleton with `package.json`, `tsconfig`, exports.
3. Add Effect contracts/errors/services.
4. Add test harness with `vitest`.

## Phase 1: L402 core + tests (Day 1-2)

1. Implement `WWW-Authenticate` parser.
2. Implement auth header builder (`Authorization: L402 ...`).
3. Implement fetch retry flow.
4. Add unit tests for parse/retry/error cases.

## Phase 2: Payment adapter and policy (Day 2-3)

1. Implement `InvoicePayer` adapter (real + demo fallback).
2. Implement spend policy checks (cap + allowlist).
3. Add integration tests with mocked 402 challenge flow.

## Phase 3: openagents.com wiring (Day 3-4)

1. Add web Effect service wrapper.
2. Add worker endpoint for L402 fetch.
3. Add one Autopilot tool contract.
4. Add minimal UI status rendering in chat.

## Phase 4: Seller routes + aperture (Day 4-5)

1. Stand up two paid endpoints.
2. Put them behind Aperture.
3. Validate with `lnget` and then with Autopilot flow.

## Phase 5: Demo polish and rehearsal (Day 5)

1. Confirm deterministic payloads.
2. Confirm one payment and one cached call.
3. Confirm one policy-denied call.
4. Capture logs/screens for episode cuts.

## 8. GitHub Issue Breakdown (EP212 Scope)

## Issue 1: Create `packages/lightning-effect` package scaffold (Effect-first)

Set up a new publishable package at `packages/lightning-effect` with baseline build/test plumbing, exports, and package metadata. This should include a clean public API surface and internal module layout so we do not leak OpenAgents app-specific code into the library.

The implementation should mirror `nostr-effect` conventions where possible: Effect-first entrypoints, tagged errors, Layer-driven composition, and test structure. This issue is complete when other packages/apps can import the new package without circular dependencies or runtime assumptions.

## Issue 2: Implement L402 contracts and parser in `lightning-effect`

Add Effect `Schema` contracts for core L402 and payment entities (`L402Challenge`, `L402Credential`, `InvoicePaymentRequest`, `InvoicePaymentResult`, and fetch request/response shapes). Then implement the `WWW-Authenticate` parser and `Authorization: L402 ...` serializer.

This issue should produce deterministic tests for success and failure cases (malformed challenge, missing invoice, missing macaroon). It is complete when the parser and serializer are unit-tested and safe to use from web/worker code paths.

## Issue 3: Add L402 fetch-retry client + credential cache service

Implement an Effect `L402Client` service that performs the full flow: initial request, challenge parse, invoice payment call, retry with auth header, and credential caching for repeated requests. Caching must be scope-aware (at least domain-level) and explicit about hit/miss behavior.

This issue is complete when integration tests demonstrate (a) first call pays and succeeds, (b) second call reuses cache, and (c) cache miss/invalid scenarios recover correctly.

## Issue 4: Implement `InvoicePayer` adapters (real + demo)

Add an `InvoicePayer` service interface and provide at least two adapters: one real adapter for Lightning Labs-oriented execution path and one deterministic demo adapter for local tests/rehearsal. The contract must require `preimageHex` on success so receipts and L402 auth remain valid.

This issue is complete when adapter behavior is validated through contract tests, including error-path tests (payment failure, timeout, missing preimage).

## Issue 5: Add spend policy guardrails (cap + allowlist) in Effect services

Implement `SpendPolicyService` so requests can be blocked before payment if they exceed max cost or violate allowlist rules. Policy decisions should be explicit and return typed denial reasons for UI and logs.

This issue is complete when policy checks are enforced in the L402 flow and tested for both allow and deny paths, including the “expensive endpoint blocked before payment” EP212 scenario.

## Issue 6: Wire `apps/web` worker endpoint to `lightning-effect`

Add `POST /api/lightning/l402/fetch` in the web worker path and back it with the `lightning-effect` services/layers. Keep execution boundaries clear: orchestration in worker, payment backend behind adapter interface.

This issue is complete when a local/staging endpoint can be called from the web app and returns typed L402 fetch results, including payment metadata and policy-deny responses.

## Issue 7: Add Autopilot tool contract and wiring for paid fetch

Add a new tool contract (for example `lightning_l402_fetch`) in `apps/autopilot-worker` with schema validation and deterministic outputs. Route tool execution through the new web endpoint so chat workflows can invoke paid endpoint consumption.

This issue is complete when Autopilot can execute a paid fetch request end-to-end from chat and emit a user-visible outcome for success or deny conditions.

## Issue 8: Add chat-visible payment states + proof reference

Add minimal UI messaging in `openagents.com` chat for payment intent, paid success, cached reuse, and blocked-by-policy outcomes. Include a proof reference field (preimage hash ref or receipt id) so the user can see that payment evidence exists.

This issue is complete when the EP212 flow is legible to viewers without opening backend logs and all four status states are visible in chat output.

## Issue 9: Stand up two demo paid endpoints behind Aperture

Create two deterministic seller endpoints (`signal-feed` and `deep-report`) and place them behind Aperture with known pricing suitable for demo behavior. Keep payloads stable and simple to avoid flakiness during recording.

This issue is complete when both endpoints can be hit via `lnget` and via Autopilot, with one endpoint under cap (success) and one endpoint over cap (policy deny).

## Issue 10: Add observability + rehearsal checklist for recording

Add structured logging fields required for the episode: request id, endpoint, quoted cost, cap, paid amount, proof ref, cache hit/miss, and denial reason. Create a short rehearsal checklist to verify all scenes before recording.

This issue is complete when a dry run produces predictable logs and the exact EP212 recording script can be executed start-to-finish without manual patching.

## 9. Demo Acceptance Criteria

EP212 is "ready" only if all pass:

1. User can trigger paid fetch from `openagents.com` chat.
2. One request performs real L402 payment and returns premium payload.
3. Second equivalent request reuses cached credential (or explicitly no new pay event).
4. Over-cap request is blocked before payment.
5. Receipt/proof reference is emitted (preimage hash ref or payment proof ref).
6. All core L402 logic is sourced from `packages/lightning-effect` (not ad hoc app code).

## 10. Observability for the Episode

Must log and be able to display:

1. request id
2. endpoint
3. quoted cost
4. cap applied
5. paid amount
6. payment proof ref
7. cache hit/miss
8. deny reason (if blocked)

## 11. Risks and Mitigations

1. Aperture/lnget setup friction
   - Mitigation: use pinned source build scripts + preflight checklist before recording.
2. Worker runtime constraints
   - Mitigation: keep heavy payment execution in adapter/executor boundary; Worker orchestrates.
3. Preimage handling mistakes
   - Mitigation: enforce `preimageHex` in contract and test failure when absent.
4. Demo flakiness from live market/data APIs
   - Mitigation: deterministic seller payloads for episode recording.

## 12. Recording Script (Short)

1. Open `openagents.com`, enter Autopilot chat.
2. Ask for premium signal feed with 100-sat cap.
3. Approve spend intent.
4. Show returned premium result + payment status.
5. Repeat request and show cached credential behavior.
6. Ask for deep report with same cap and show policy block.
7. Close with: "This capability came from user requests in EP211; now live in Autopilot."

## 13. Post-EP212 Follow-Up

After shipping episode demo:

1. Expand `lightning-effect` docs/examples for external users.
2. Add second adapter path (Spark/Breez-compatible payer) under same interfaces.
3. Extend from buyer-only demo to seller-side OpenAgents endpoint monetization playbook.
