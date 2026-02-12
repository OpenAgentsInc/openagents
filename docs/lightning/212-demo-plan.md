# EP212 Demo Plan: L402 Buying on openagents.com (Agent-Owned Spark Wallet + sats4ai + OpenAgents Aperture/Voltage)

Status: Draft
Date: 2026-02-12
Owner: OpenAgents

## 1. Demo Goal

Show, inside `openagents.com`, that Autopilot can consume paid endpoints using Lightning Labs L402 flows with clear budget guardrails and receipts, using a real Lightning wallet owned by the agent.

Narrative continuation from EP211:

- EP211: "ask for capabilities, they get added quickly"
- EP212: "we added paid API consumption over Lightning"

EP212 framing:

- L402 is "pay-per-use over HTTP": macaroons (token) + Lightning invoice (payment) + preimage (proof) = access.
- We will demonstrate:
  - Paying a third-party L402 API (`sats4ai.com`) from the agent wallet.
  - Paying an OpenAgents-hosted L402 endpoint (Aperture + Voltage) from the same wallet.

## 2. Hard Requirements for EP212

1. Effect-first architecture only.
2. New reusable package: `packages/lightning-effect` (usable by other projects).
3. Demo runs through `openagents.com` chat UX (not a standalone CLI-only demo).
4. **No desktop dependency for EP212**: payment execution runs in OpenAgents infra, not on the viewer’s machine.
5. Autopilot uses an **agent-owned Spark wallet** (Breez Spark) with keys stored in an OpenAgents secret manager.
6. Show **live wallet balance** on screen during the chat (Spark wallet status).
7. Consume at least two L402 endpoints:
   - Third-party: `sats4ai.com` (text generation).
   - OpenAgents: `l402.openagents.com` (Aperture + Voltage) route.
8. Show one successful payment and one policy guardrail event.
9. Show receipts/proof reference in product UI/logs (payment id, amount, cache hit, response hash).

## 3. Scope for This Episode (and only this episode)

## In scope

- Buyer-side L402 flow (402 challenge -> pay -> retry -> response).
- Third-party paid endpoint integration: `https://sats4ai.com/api/l402/text-generation`.
- OpenAgents-hosted L402 endpoint: **Aperture** on Cloud Run connected to **Voltage** LND + Cloud SQL Postgres.
- Agent-owned Spark wallet execution in OpenAgents infra (server-side) for payments.
- Chat-visible payment state in `openagents.com`.
- Budget cap + allowlist guardrails.
- `lightning-effect` package core (L402 parsing, policy, retry, caching).
- Minimal web panes for wallet + transactions + payment detail (episode legibility).

## Out of scope

- A full self-custodial user wallet experience in the browser.
- Full seller marketplace.
- Mobile/desktop full parity.
- Production-grade multi-tenant billing/reconciliation.

## 4. Demo User Story

User in Autopilot chat on `openagents.com`:

1. "Use L402 to call a paid API. Max 100 sats. Explain what’s happening."
2. Autopilot shows spend intent and asks for approval (cap + domain).
3. User approves.
4. Autopilot calls `sats4ai` text-generation behind L402, returns result, and shows payment receipt details.
5. User repeats the same request; Autopilot reuses cached credential (no second payment).
6. Autopilot calls an OpenAgents-hosted L402 endpoint (Aperture + Voltage) and returns premium payload summary.
7. User requests an over-cap route; Autopilot blocks **before payment** with a clear policy reason.

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
   - Shows wallet/transactions/payment panes for the agent wallet.
2. OpenAgents Worker path (`apps/web` Worker surface)
   - Validates tool request schemas.
   - Applies preflight policy checks.
   - Orchestrates the L402 fetch tool call.
3. Convex (`apps/web/convex`)
   - Stores durable receipts/logs for UI replay (payment attempts, cache hit/miss, policy decisions).
   - Stores agent wallet “status snapshot” rows used by panes (balance, last payment, last error).
4. **Agent wallet executor (Spark)** (new service; server-side)
   - Runs Breez Spark SDK (Node runtime) and implements:
     - `payBolt11` (returns preimage)
     - `walletStatus` (balance, payments)
   - Exposes a narrow RPC API (HTTP) to the Worker/autopilot tool layer.
   - Loads seed material from secret manager; never ships keys to the browser.
5. Third-party L402 seller: `sats4ai.com`
   - Paid endpoint: `https://sats4ai.com/api/l402/text-generation`
6. **OpenAgents L402 gateway** (seller/paywall)
   - **Aperture** deployed on **GCP Cloud Run**, connected to **Voltage LND** (gRPC) and **Cloud SQL Postgres**.
   - Emits `402 Payment Required` challenges (invoice + macaroon) and proxies authenticated requests to upstream.
7. Upstream demo backend (behind the paywall)
   - Any normal HTTP service (Cloud Run is fine). Not L402-aware; Aperture handles L402 for it.

Security boundary for EP212:

1. No wallet keys, macaroon secrets, or preimages are persisted in browser code.
2. Wallet seed material is stored only in server-side secret manager and used only by the wallet executor service.
3. The Worker/autopilot tool layer never receives the seed phrase; it receives payment results (preimage) only as needed for L402 authorization.

## 5.3 openagents.com + Autopilot integration points

1. Autopilot tool:
   - `lightning_l402_fetch` executes a paid HTTP request using the agent Spark wallet.
   - It must support a hard spend cap (`maxSpendSats`) + allowlist.
2. Wallet executor RPC:
   - `POST /api/lightning/spark/pay-bolt11`
   - `GET /api/lightning/spark/status`
   - Optional combined: `POST /api/lightning/l402/fetch` (executes the full 402->pay->retry).
3. UI surface:
   - Chat part/status lines for:
   - `payment.intent`
   - `payment.sent`
   - `payment.cached`
   - `payment.blocked`
   - `payment.failed`
4. Panes:
   - Wallet (Spark balance + last payments)
   - Transactions (L402 attempts across sats4ai + OA gateway)
   - Payment detail (invoice, preimage hash ref, response hash, cache hit)

## 5.4 Endpoint Selection for Demo

Use two L402 endpoints with different trust domains:

1. Endpoint A (third-party, “explain L402” segment):
   - `POST https://sats4ai.com/api/l402/text-generation`
   - Demonstrate the two-step L402 shape (402 challenge → pay invoice → retry with preimage)
2. Endpoint B (OpenAgents gateway, deterministic):
   - `https://l402.openagents.com/...` (Aperture)
   - One route under cap (success) and one route over cap (blocked pre-payment after quoting invoice)

Selection requirements:

1. sats4ai request/response is stable enough to rehearse and record.
2. OpenAgents gateway domain + routes are stable and controlled by OpenAgents.
3. Host matching works in browser without custom headers:
   - Prefer `l402.openagents.com` mapped to Cloud Run.
   - Ensure Aperture `services.host` matches the domain used in the demo.

References:

- Deploy/runbook: `docs/lightning/L402_APERTURE_DEPLOY_RUNBOOK.md`
- Voltage wiring: `docs/lightning/VOLTAGE_TO_L402_CONNECT.md`

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

## Endpoint A: sats4ai text generation (paid, third-party)

We will mirror the “Understanding L402” story in the episode:

1. Initial request returns `402 Payment Required` with:
   - `www-authenticate: L402 macaroon="...", invoice="..."`
2. The wallet pays the invoice and returns a preimage.
3. The same request is repeated with:
   - `Authorization: L402 <macaroon>:<preimage>` (sats4ai docs format)

Implementation note:

- `packages/lightning-effect` currently serializes:
  - `Authorization: L402 macaroon="...", preimage="..."`
- For EP212 we must support sats4ai’s accepted format, either by:
  - host-specific header serialization, or
  - trying both formats on retry when the first fails (bounded, deterministic).

Expected:

- User approves spend cap.
- Payment succeeds and the response is returned in chat.
- Receipt metadata is shown (amount, payment id, response hash).

## Endpoint A repeat call: sats4ai cache hit

- Same domain + credential scope
- Expected: cached L402 credential reuse; no new payment

## Endpoint B: OpenAgents Aperture + Voltage (paid, deterministic)

We will have two routes behind `l402.openagents.com`:

1. Endpoint B1 (success): priced under cap.
2. Endpoint B2 (block): priced above cap so we can show a clean deny path after seeing the quoted invoice amount.

Expected:

- One paid request succeeds and returns a small “premium payload” (JSON) that Autopilot summarizes.
- One request is blocked before payment with: `quoted > cap`.

## 7. Implementation Sequence (Tight)

## Phase 0: Confirm seller infra baseline (Day 0)

1. Aperture Cloud Run service is healthy (see `docs/lightning/L402_APERTURE_DEPLOY_RUNBOOK.md`).
2. Domain is mapped:
   - Prefer `l402.openagents.com` (Cloud Run custom domain mapping).
3. Voltage credentials are mounted and Postgres is healthy.

## Phase 1: Spark wallet executor service (Day 1)

1. Create a new server-side service that can run Breez Spark SDK (Node runtime), written in Effect.
2. Store the agent seed phrase in secret manager (one wallet for EP212; extend to more agents later).
3. Expose minimal RPC endpoints:
   - `GET /status` (balance + last payments)
   - `POST /pay-bolt11` (returns preimage)
   - Optional combined: `POST /l402/fetch` (runs 402->pay->retry in one place)
4. Add rate limits and caps in-service (defense in depth).

## Phase 2: L402 header compatibility for sats4ai (Day 1-2)

1. Support `Authorization: L402 <macaroon>:<preimage>` for `sats4ai.com`.
2. Keep spec form working: `Authorization: L402 macaroon="...", preimage="..."`
3. Add deterministic unit tests.

## Phase 3: Autopilot tool + receipts (Day 2-3)

1. Tool `lightning_l402_fetch`:
   - enforce `maxSpendSats`
   - enforce allowlist
   - require user approval before paying
2. Execute paid fetch using the server-side Spark wallet executor.
3. Persist receipts + wallet snapshot rows into Convex for UI.

## Phase 4: Web UI panes + Storybook (Day 3-4)

1. Wallet pane shows Spark balance + last payment summary.
2. Transactions pane shows last N L402 attempts (domain, sats, status, cache hit).
3. Payment detail pane shows invoice + quoted cost + cap + response hash/preview.
4. Add Storybook stories for all key states (offline, paying, paid, cached, blocked, failed).

## Phase 5: OpenAgents Aperture demo routes (Day 4-5)

1. Stand up a tiny upstream “premium payload” service (Cloud Run ok).
2. Configure Aperture routes on `l402.openagents.com`:
   - B1 under cap: returns premium JSON
   - B2 over cap: same upstream, higher price
3. Validate end-to-end (challenge → pay → proxy).

## Phase 6: Rehearsal + deterministic test harness (Day 5-6)

1. Add a local L402 test server that can emulate sats4ai’s header expectations.
2. Add a smoke script that runs the EP212 sequence programmatically.
3. Rehearse on production with real sats4ai + `l402.openagents.com` routes.

## 8. GitHub Issue Breakdown (EP212 Scope)

## Epic: EP212 L402 Buying Demo (Agent-Owned Spark Wallet)

This epic tracks everything needed to do the demo entirely inside `openagents.com` without a desktop payer, using:

- third-party L402: `sats4ai.com`
- OpenAgents seller gateway: Aperture + Voltage (`l402.openagents.com`)

### Issue A: Add server-side Spark wallet executor (agent-owned)

Create a new Effect service (Cloud Run or equivalent Node runtime) that runs Breez Spark SDK and holds the agent wallet seed in secret manager. It must expose a minimal API for balance + paying BOLT11 invoices, and it must implement hard spend caps and allowlists defensively.

This issue is complete when `apps/web` (Worker) can query wallet status and pay a BOLT11 invoice via the executor without any browser-exposed secrets.

### Issue B: `lightning-effect` sats4ai header compatibility

Update `packages/lightning-effect` to support sats4ai’s authorization header format (`Authorization: L402 <macaroon>:<preimage>`) while keeping the spec key/value format working. Add deterministic tests, and make the behavior host-configurable (do not guess globally).

### Issue C: Autopilot tool `lightning_l402_fetch` uses the Spark wallet executor

Wire the tool path so paid fetch runs entirely server-side using the agent wallet executor:

- approval gating
- allowlist + cap enforcement
- response body capture (bounded) + receipts persisted to Convex for panes

### Issue D: Wallet + transactions panes (plus Storybook stories)

Implement L402-focused panes using the existing `effuse-panes` integration in `openChatPaneController`: a persistent wallet pane (`l402-wallet`), a persistent transactions pane (`l402-transactions`), and on-demand payment detail panes (`l402-payment-*`). Pane creation/open/close should follow existing `store.addPane`/`store.togglePane` + `calculateNewPanePosition` patterns used by metadata/telemetry panes.

This issue should also add trigger controls (chat title actions and/or message-level buttons), while preserving current overlay behavior (closing non-chat panes does not dismiss overlay). It is complete when the three pane types can be opened reliably and render L402-specific data from the same source used by chat.

### Issue E: OpenAgents Aperture demo routes + upstream premium backend

Deploy a small upstream premium backend and configure Aperture routes on `l402.openagents.com` for:

- under-cap success
- over-cap block

Validate end-to-end against Voltage (invoice issuance + settlement).

### Issue F: Deterministic test harness + smoke script for the full EP212 flow

Create a deterministic programmatic harness that exercises:

- sats4ai-like L402 (local test server with same header behavior)
- OpenAgents gateway route (staging/prod smoke)

and gates CI on “no regressions” for L402 buying.

Add a rehearsal checklist for recording day.

## 9. Demo Acceptance Criteria

EP212 is "ready" only if all pass:

1. User can trigger paid fetch from `openagents.com` chat.
2. Agent Spark wallet is funded and its balance is visible in panes.
3. sats4ai request performs real L402 payment and returns the text-generation payload.
4. Repeating sats4ai call shows cache hit and no second payment.
5. OpenAgents `l402.openagents.com` paid route succeeds and returns premium JSON.
6. OpenAgents over-cap route is blocked before payment.
7. All core L402 logic is sourced from `packages/lightning-effect` (not ad hoc app code).
8. Wallet and transactions panes open in the overlay and show data consistent with chat/payment events.

## 10. Observability for the Episode

Must log and be able to display:

1. request id
2. user id
3. agent wallet id
4. endpoint
5. quoted cost
6. cap applied
7. paid amount
8. payment proof ref
9. cache hit/miss
10. deny reason (if blocked)
11. executor (`spark-service`)

## 11. Risks and Mitigations

1. Spark wallet executor offline
   - Mitigation: health check + show “wallet offline” in panes; block paid tool calls when offline.
2. Gateway routing / config mismatch (Host not matching; route missing; pricing not deterministic)
   - Mitigation: map `l402.openagents.com`, pin the demo routes, and rehearse using the same URL the chat uses.
3. Worker/Convex orchestration drift
   - Mitigation: typed task state machine + replayable status transitions in tests.
4. Preimage handling mistakes
   - Mitigation: enforce `preimageHex` in contract and test failure when absent.
5. Voltage connectivity or Postgres issues (Aperture can’t issue challenges)
   - Mitigation: run the smoke/reconcile flows from the runbook before recording; keep logs visible during rehearsal.

## 12. Recording Script (Short)

1. Confirm the agent Spark wallet is funded and the wallet executor is healthy.
2. Open `openagents.com`, enter Autopilot chat.
3. Ask for sats4ai text-generation with a 100-sat cap; narrate L402 (402 challenge → invoice → preimage).
4. Approve spend intent.
5. Show returned sats4ai result + payment receipt.
6. Repeat sats4ai request and show cached credential behavior.
7. Call the OpenAgents L402 endpoint on `l402.openagents.com` and show the premium JSON summary.
8. Call the over-cap OpenAgents route and show policy block.
9. Close with: "This capability came from user requests in EP211; now live in Autopilot."

## 13. Post-EP212 Follow-Up

After shipping episode demo:

1. Expand `lightning-effect` docs/examples for external users.
2. Add second adapter path (Spark/Breez-compatible payer) under same interfaces.
3. Expand coverage from the EP212 demo routes to additional OpenAgents-hosted paywalled routes and/or third-party L402 sellers.
