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
4. Payment execution and wallet access run from a desktop companion app (Electron), not browser/Worker.
5. Web app and desktop app are correlated by the same authenticated OpenAgents user via Convex.
6. Consume at least two paid endpoints.
7. Show one successful payment and one policy guardrail event.
8. Show receipts/proof reference in product UI/logs.

## 3. Scope for This Episode (and only this episode)

## In scope

- Buyer-side L402 flow (402 challenge -> pay -> retry -> response).
- Integration with existing paid seller endpoints (external to OpenAgents).
- Electron desktop companion app setup for wallet/payment execution.
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

## 5.2 Runtime Topology (Where Each Piece Runs)

1. `openagents.com` browser app (`apps/web`)
   - Captures user intent in chat.
   - Shows payment states and proof references.
   - Renders wallet/transactions/payment panes from task status.
2. OpenAgents Worker path (`apps/web` Worker surface)
   - Validates tool request schemas.
   - Applies preflight policy checks.
   - Writes an execution task to Convex for the same authenticated user.
3. Convex (`apps/web/convex`)
   - Acts as command/result bus.
   - Stores task lifecycle (`queued`, `approved`, `running`, `paid`, `cached`, `blocked`, `failed`, `completed`).
   - Stores proof references and denial reasons for replay in web UI/panes.
4. Desktop companion app (new Electron app)
   - Runs `lightning-effect` live layers plus payment adapter.
   - Owns wallet/payment execution and L402 challenge handling.
   - Polls/subscribes for assigned user tasks, executes them, writes result back to Convex.
5. External seller endpoints
   - Existing L402-gated APIs only (no OpenAgents-hosted seller infra in EP212).

Security boundary for EP212:

1. No wallet keys, macaroon secrets, or preimages are persisted in browser code.
2. Worker and Convex carry references/metadata; sensitive wallet execution remains in desktop app.
3. Remote signer split is a post-EP212 hardening step; EP212 demo can run single desktop app mode.

## 5.3 openagents.com + Autopilot integration points

1. `apps/web/src/effect/lightning.ts`
   - Client-side Effect service for task creation/status subscription.
2. Worker endpoint:
   - `POST /api/lightning/l402/tasks`
   - Creates task records only (does not execute wallet payment).
3. Autopilot path:
   - Add one tool contract (`lightning_l402_fetch`) that schedules a task and awaits status updates.
4. UI surface:
   - Chat part/status lines for:
   - `payment.intent`
   - `payment.sent`
   - `payment.cached`
   - `payment.blocked`
   - `payment.failed`

## 5.4 Existing Endpoint Selection for Demo

Use at least two existing L402-gated seller endpoints (not owned or hosted by OpenAgents):

1. Endpoint A (target happy path):
   - priced under demo cap (example cap: 100 sats)
2. Endpoint B (target deny path):
   - priced above demo cap (or policy-disallowed domain)

Selection requirements:

1. Endpoint URLs and price behavior are known before recording day.
2. Endpoints are stable enough for rehearsal and capture.
3. We do not build or host seller infrastructure in EP212 scope.

## 5.5 Pane System Plan (effuse-panes + apps/web)

Current baseline in `apps/web`:

1. Pane lifecycle is mounted through an Effect service (`apps/web/src/effect/paneSystem.ts`) using scoped acquire/release.
2. Home overlay pane config currently disables hotbar (`enableHotbar: false`) and uses one primary pane (`home-chat`) plus ad hoc `metadata-*` and `telemetry-*` panes from message actions.
3. Pane placement uses `calculateNewPanePosition(store.lastPanePosition, ...)`, and the main chat pane persists position to local storage.

EP212 pane additions (demo scope):

1. Add a persistent wallet summary pane:
   - id: `l402-wallet`
   - kind: `l402-wallet`
   - title: `Wallet`
   - purpose: balance, spend cap, allowlist status, last paid endpoint
2. Add a persistent transactions pane:
   - id: `l402-transactions`
   - kind: `l402-transactions`
   - title: `Transactions`
   - purpose: recent L402 attempts with status (`paid`, `cached`, `blocked`, `failed`)
3. Add on-demand payment detail pane:
   - id prefix: `l402-payment-`
   - kind: `l402-payment`
   - title: `Payment Detail`
   - purpose: request id, endpoint, quoted cost, paid amount, proof reference, policy decision

Interaction model for EP212:

1. Keep `home-chat` as primary pane and avoid replacing existing metadata/telemetry flows.
2. Add title action buttons on `home-chat` for opening/toggling `l402-wallet` and `l402-transactions`.
3. Add a per-message/action button for `Payment Detail` when a message contains L402/payment metadata.
4. Optional operator mode: enable hotbar by feature flag and map slots (for example `1=Chat`, `2=Wallet`, `3=Transactions`) using `onHotbarSlotClick` + `store.togglePane`.

Rendering and state constraints:

1. Continue host-rendering into `[data-pane-id=\"<id>\"] [data-oa-pane-content]` after `paneSystem.render()`.
2. Reuse existing `stylePaneOpaqueBlack` behavior for visual consistency in EP212 recordings.
3. Persist `l402-wallet` and `l402-transactions` rects via `store.closedPositions` reopen semantics; local-storage persistence beyond episode scope is optional.
4. `onPaneClosed` behavior remains: only closing `home-chat` dismisses the overlay; closing L402 panes should not close chat.

## 6. Endpoint Consumption Plan (Demo-Focused)

## Endpoint A (happy path)

- Cap: 100 sats
- Price: below cap
- Expected: payment approved + success + summary

## Endpoint A repeat call (cache path)

- Same domain + credential scope
- Expected: cached L402 credential reuse; no new payment

## Endpoint B (policy deny path)

- Cap: 100 sats
- Price: above cap or blocked by allowlist policy
- Expected: blocked before payment with clear explanation

## 7. Implementation Sequence (Tight)

## Phase 0: Bootstrap Electron desktop companion app (Day 1)

1. Create new app shell (for example `apps/desktop`) with Electron + Effect runtime.
2. Add OpenAgents auth flow and Convex connection for the signed-in user.
3. Implement background task loop skeleton (`queued` -> `running` -> `completed|failed`).

## Phase 1: Scaffold `lightning-effect` (Day 1-2)

1. Run Effect guidance preflight:
   - `effect-solutions list`
   - `effect-solutions show quick-start basics services-and-layers error-handling testing`
2. Create package skeleton with `package.json`, `tsconfig`, exports.
3. Add Effect contracts/errors/services.
4. Add test harness with `vitest`.

## Phase 2: L402 core + tests (Day 2-3)

1. Implement `WWW-Authenticate` parser.
2. Implement auth header builder (`Authorization: L402 ...`).
3. Implement fetch retry flow.
4. Add unit tests for parse/retry/error cases.

## Phase 3: Payment adapter and policy (Day 3-4)

1. Implement `InvoicePayer` adapter (real + demo fallback).
2. Implement spend policy checks (cap + allowlist).
3. Add integration tests with mocked 402 challenge flow.

## Phase 4: Web/Worker/Convex task wiring (Day 4-5)

1. Add web Effect service wrapper for task creation/subscription.
2. Add worker endpoint for task creation (`/api/lightning/l402/tasks`).
3. Add Convex schema/functions for task lifecycle state transitions.
4. Add one Autopilot tool contract that awaits task completion.

## Phase 5: UI panes + chat status integration (Day 5-6)

1. Add wallet/transactions/payment detail panes.
2. Feed pane/chat states from Convex task updates.
3. Confirm pane interactions in home overlay.

## Phase 6: Existing endpoint integration + validation (Day 6)

1. Finalize two existing seller endpoints for demo.
2. Validate both directly (for example with `lnget` from desktop) and through Autopilot flow.
3. Confirm happy path + deny path behavior is repeatable.

## Phase 7: Demo polish and rehearsal (Day 6-7)

1. Confirm desktop companion is online and bound to the same user account as web.
2. Confirm selected external endpoints are stable at recording time.
3. Confirm one payment and one cached call.
4. Confirm one policy-denied call.
5. Capture logs/screens for episode cuts.

## 8. GitHub Issue Breakdown (EP212 Scope)

## Issue 1: Create Electron desktop companion app shell (`apps/desktop`)

Create a new Electron app with an Effect runtime boundary that can authenticate as an OpenAgents user and connect to Convex. The app should include a background task executor loop for Lightning jobs and a minimal status UI that shows connectivity and current task state.

This issue is complete when the desktop app can sign in, subscribe/poll for task records for that same user, and write state updates back to Convex without any Lightning-specific logic yet.

## Issue 2: Create `packages/lightning-effect` package scaffold (Effect-first)

Set up a new publishable package at `packages/lightning-effect` with baseline build/test plumbing, exports, and package metadata. This should include a clean public API surface and internal module layout so we do not leak OpenAgents app-specific code into the library.

The implementation should mirror `nostr-effect` conventions where possible: Effect-first entrypoints, tagged errors, Layer-driven composition, and test structure. This issue is complete when other packages/apps (including the desktop app) can import the new package without circular dependencies or runtime assumptions.

## Issue 3: Implement L402 contracts and parser in `lightning-effect`

Add Effect `Schema` contracts for core L402 and payment entities (`L402Challenge`, `L402Credential`, `InvoicePaymentRequest`, `InvoicePaymentResult`, and fetch request/response shapes). Then implement the `WWW-Authenticate` parser and `Authorization: L402 ...` serializer.

This issue should produce deterministic tests for success and failure cases (malformed challenge, missing invoice, missing macaroon). It is complete when the parser and serializer are unit-tested and safe to use from web/worker code paths.

## Issue 4: Add L402 fetch-retry client + credential cache service

Implement an Effect `L402Client` service that performs the full flow: initial request, challenge parse, invoice payment call, retry with auth header, and credential caching for repeated requests. Caching must be scope-aware (at least domain-level) and explicit about hit/miss behavior.

This issue is complete when integration tests demonstrate (a) first call pays and succeeds, (b) second call reuses cache, and (c) cache miss/invalid scenarios recover correctly.

## Issue 5: Implement `InvoicePayer` adapters (real + demo)

Add an `InvoicePayer` service interface and provide at least two adapters: one real adapter for Lightning Labs-oriented execution path and one deterministic demo adapter for local tests/rehearsal. The contract must require `preimageHex` on success so receipts and L402 auth remain valid.

This issue is complete when adapter behavior is validated through contract tests, including error-path tests (payment failure, timeout, missing preimage).

## Issue 6: Add spend policy guardrails (cap + allowlist) in Effect services

Implement `SpendPolicyService` so requests can be blocked before payment if they exceed max cost or violate allowlist rules. Policy decisions should be explicit and return typed denial reasons for UI and logs.

This issue is complete when policy checks are enforced in the L402 flow and tested for both allow and deny paths, including the “expensive endpoint blocked before payment” EP212 scenario.

## Issue 7: Wire Worker + Convex task orchestration for desktop execution

Add `POST /api/lightning/l402/tasks` in the web Worker path and back it with task orchestration logic that writes to Convex for the active user. Keep execution boundaries clear: orchestration in Worker, payment execution in desktop companion.

This issue is complete when web requests create typed execution tasks and the desktop companion can pick them up and return typed results, including payment metadata and policy-deny responses.

## Issue 8: Add Autopilot tool contract and wiring for paid fetch

Add a new tool contract (for example `lightning_l402_fetch`) in `apps/autopilot-worker` with schema validation and deterministic outputs. Route tool execution through task creation + task status wait so chat workflows invoke paid endpoint consumption through the desktop executor.

This issue is complete when Autopilot can execute a paid fetch request end-to-end from chat and emit a user-visible outcome for success or deny conditions.

## Issue 9: Add wallet + transactions panes in home chat overlay

Implement L402-focused panes using the existing `effuse-panes` integration in `openChatPaneController`: a persistent wallet pane (`l402-wallet`), a persistent transactions pane (`l402-transactions`), and on-demand payment detail panes (`l402-payment-*`). Pane creation/open/close should follow existing `store.addPane`/`store.togglePane` + `calculateNewPanePosition` patterns used by metadata/telemetry panes.

This issue should also add trigger controls (chat title actions and/or message-level buttons), while preserving current overlay behavior (closing non-chat panes does not dismiss overlay). It is complete when the three pane types can be opened reliably and render L402-specific data from the same source used by chat.

## Issue 10: Add chat-visible payment states + proof reference

Add minimal UI messaging in `openagents.com` chat for payment intent, paid success, cached reuse, and blocked-by-policy outcomes. Include a proof reference field (preimage hash ref or receipt id) so the user can see that payment evidence exists.

This issue is complete when the EP212 flow is legible to viewers without opening backend logs and all four status states are visible in chat output.

## Issue 11: Integrate two existing paid seller endpoints for demo

Select and integrate two existing L402-gated endpoints so EP212 can demonstrate both success and policy-denied behavior without standing up OpenAgents-hosted seller infrastructure. Document endpoint assumptions (price range, reliability, request shape) inside the implementation notes and rehearsal checklist.

This issue is complete when both endpoints can be hit via `lnget` in the desktop app environment and via Autopilot, with one endpoint under cap (success) and one endpoint over cap or policy-blocked (deny).

## Issue 12: Add observability + rehearsal checklist for recording

Add structured logging fields required for the episode: request id, user id, endpoint, quoted cost, cap, paid amount, proof ref, cache hit/miss, denial reason, and execution location (`desktop`). Create a short rehearsal checklist to verify all scenes before recording.

This issue is complete when a dry run produces predictable logs and the exact EP212 recording script can be executed start-to-finish without manual patching.

## 9. Demo Acceptance Criteria

EP212 is "ready" only if all pass:

1. User can trigger paid fetch from `openagents.com` chat.
2. Desktop companion is authenticated as the same user and online.
3. One request performs real L402 payment and returns premium payload.
4. Second equivalent request reuses cached credential (or explicitly no new pay event).
5. Over-cap request is blocked before payment.
6. Receipt/proof reference is emitted (preimage hash ref or payment proof ref).
7. All core L402 logic is sourced from `packages/lightning-effect` (not ad hoc app code).
8. Wallet and transactions panes open in the overlay and show data consistent with chat/payment events.

## 10. Observability for the Episode

Must log and be able to display:

1. request id
2. user id
3. endpoint
4. quoted cost
5. cap applied
6. paid amount
7. payment proof ref
8. cache hit/miss
9. deny reason (if blocked)
10. executor (`desktop`)

## 11. Risks and Mitigations

1. Desktop companion offline or not authenticated
   - Mitigation: explicit connection health in web UI + preflight check before task submission.
2. External endpoint reliability/availability
   - Mitigation: pre-qualify multiple endpoint candidates and keep a backup endpoint list for recording day.
3. Worker/Convex orchestration drift
   - Mitigation: typed task state machine + replayable status transitions in tests.
4. Preimage handling mistakes
   - Mitigation: enforce `preimageHex` in contract and test failure when absent.
5. Demo flakiness from third-party response variability
   - Mitigation: constrain prompt/output expectations to endpoint metadata and summary quality rather than exact payload text.

## 12. Recording Script (Short)

1. Start desktop companion app and confirm user session connected.
2. Open `openagents.com`, enter Autopilot chat with same user account.
3. Ask for premium signal feed with 100-sat cap.
4. Approve spend intent.
5. Show returned premium result + payment status.
6. Repeat request and show cached credential behavior.
7. Ask for deep report with same cap and show policy block.
8. Close with: "This capability came from user requests in EP211; now live in Autopilot."

## 13. Post-EP212 Follow-Up

After shipping episode demo:

1. Expand `lightning-effect` docs/examples for external users.
2. Add second adapter path (Spark/Breez-compatible payer) under same interfaces.
3. Expand coverage from two endpoints to a broader catalog of existing L402 seller endpoints.
