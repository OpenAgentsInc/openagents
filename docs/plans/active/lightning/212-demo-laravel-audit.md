# EP212 Laravel Audit (apps/openagents.com)

Status: Active audit of what is implemented in `apps/openagents.com` for EP212-style L402 demo flows.
Date: 2026-02-17
Owner: OpenAgents

## 1) Scope Of This Audit

This audit covers the current Laravel rebuild implementation in `apps/openagents.com`, specifically:

- Autopilot chat + tool execution path.
- L402 payment flow with approval gating.
- Per-user Spark wallet integration.
- L402 UI pages and API surfaces.
- What is missing vs the original EP212 plan.

It supersedes legacy Convex + desktop executor assumptions.

## 2) What Is Implemented Now (Laravel)

### 2.1 Chat + Tool Runtime

Implemented:

- Chat streaming route and run persistence are in Laravel (`RunOrchestrator`).
- Autopilot tool registry includes:
  - `lightning_l402_fetch`
  - `lightning_l402_approve`
- Tool results for L402 are persisted as run events (`l402_fetch_receipt`) with receipt metadata.

Key files:

- `apps/openagents.com/app/AI/RunOrchestrator.php`
- `apps/openagents.com/app/AI/Agents/AutopilotAgent.php`
- `apps/openagents.com/app/AI/Tools/ToolRegistry.php`
- `apps/openagents.com/app/AI/Tools/LightningL402FetchTool.php`
- `apps/openagents.com/app/AI/Tools/LightningL402ApproveTool.php`

### 2.2 L402 Protocol Flow

Implemented:

- Host allowlist enforcement before payment.
- Challenge parse (`402` + `www-authenticate` L402 challenge).
- Quote extraction from BOLT11.
- Cap enforcement (`quoted_cost_exceeds_cap`) before payment.
- Invoice payment through configured payer.
- Retry with `Authorization: L402 <macaroon>:<preimage>`.
- Credential cache (host + scope) with TTL.
- Response capture with bounded bytes + preview + SHA256.

Key files:

- `apps/openagents.com/app/Lightning/L402/L402Client.php`
- `apps/openagents.com/app/Lightning/L402/L402CredentialCache.php`
- `apps/openagents.com/app/Lightning/L402/WwwAuthenticateParser.php`
- `apps/openagents.com/config/lightning.php`

### 2.3 Approval Gate

Implemented:

- `lightning_l402_fetch` defaults `approvalRequired=true`.
- Tool queues pending approval task in DB.
- `lightning_l402_approve` consumes task and enforces user-task match.
- Expired/missing task behavior returns deterministic deny codes (`task_not_found`, `task_expired`, etc.).

Key files:

- `apps/openagents.com/app/Lightning/L402/PendingL402ApprovalStore.php`
- `apps/openagents.com/app/AI/Tools/LightningL402FetchTool.php`
- `apps/openagents.com/app/AI/Tools/LightningL402ApproveTool.php`

### 2.4 Per-User Spark Wallet (Agent Payments API)

Implemented:

- One Spark wallet row per user.
- Wallet provisioning/import.
- Balance sync.
- Invoice creation.
- Pay BOLT11.
- Send Spark transfer.
- Stored mnemonic is encrypted at rest in DB model layer.

Key files:

- `apps/openagents.com/app/Models/UserSparkWallet.php`
- `apps/openagents.com/app/Lightning/Spark/UserSparkWalletService.php`
- `apps/openagents.com/app/Lightning/Spark/SparkExecutorClient.php`
- `apps/openagents.com/app/Http/Controllers/Api/AgentPaymentsController.php`
- `apps/openagents.com/config/lightning.php`

### 2.5 L402 Payer Binding

Implemented:

- Invoice payer is configurable:
  - `spark_wallet`
  - `lnd_rest`
  - `fake`
- Current default config is `spark_wallet`.

Key file:

- `apps/openagents.com/app/Providers/AppServiceProvider.php`

### 2.6 L402 UI Pages

Implemented authenticated pages:

- `/l402` (wallet summary)
- `/l402/transactions`
- `/l402/transactions/{eventId}`
- `/l402/paywalls`
- `/l402/settlements`
- `/l402/deployments`

Key files:

- `apps/openagents.com/routes/web.php`
- `apps/openagents.com/app/Http/Controllers/L402PageController.php`
- `apps/openagents.com/resources/js/pages/l402/*`

### 2.7 L402 API Surfaces

Implemented authenticated API endpoints:

- `GET /api/l402/wallet`
- `GET /api/l402/transactions`
- `GET /api/l402/transactions/{eventId}`
- `GET /api/l402/paywalls`
- `GET /api/l402/settlements`
- `GET /api/l402/deployments`

Note: these are read/analytics surfaces based on run events and wallet snapshot state.

## 3) Explicit Answer: Can Agents Create Paywalls Via API?

Short answer: **No, not currently in `apps/openagents.com`.**

What exists now:

- Read-only paywall analytics endpoint: `GET /api/l402/paywalls`.
- It aggregates historical receipts by host/scope.

What does not exist:

- No `POST /api/l402/paywalls` or equivalent mutation route.
- No API in Laravel app that provisions/updates Aperture routes/prices.
- No direct "agent creates seller paywall" action in the current toolset.

Operationally today, seller paywalls are configured outside this app (Aperture/GCP runbooks and config updates).

## 4) EP212 Fit Assessment (Laravel)

### 4.1 Ready for demo in current app

Ready:

- Approval-gated paid fetch flow in chat.
- Receipt event capture with status and proof reference.
- Wallet + transactions + paywall/settlement/deployment pages.
- Per-user Spark wallet API for funding/inspection/payment operations.

### 4.2 Gaps / Caveats

Not yet implemented as first-class product features:

- API-managed paywall creation/update (seller provisioning).
- Rich policy UI for allowlist/caps management (config-driven currently).
- Fully automated seller infra control from agent tools.

## 5) Recommended Demo Scope

Primary EP212 demo in `openagents.com` should show:

1. User asks for paid fetch with strict cap.
2. Agent queues approval and user approves.
3. Payment executes (Spark backend), paid response returns.
4. Receipt/proof reference shown in chat and visible on L402 pages.
5. Repeat request shows cache behavior.
6. Over-cap/disallowed scenario blocks pre-payment with clear reason.

## 6) Optional Extra APIs Worth Showing In Demo (If Time)

Good optional additions after core EP212 path:

- `GET /api/l402/wallet` to show aggregate paid/cached/blocked counts.
- `GET /api/l402/transactions` to show per-attempt receipt history.
- `POST /api/agent-payments/invoice` + `POST /api/agent-payments/pay` for explicit wallet operations.

Lower priority for EP212 narrative:

- `shouts` / `whispers` comms APIs (useful platform feature, not central to Lightning story).

## 7) Validation Artifacts In Repo

Relevant test areas already present:

- `apps/openagents.com/tests/Feature/L402ReceiptEventsTest.php`
- `apps/openagents.com/tests/Feature/Api/V1/L402ApiTest.php`
- `apps/openagents.com/tests/Feature/Api/V1/AgentPaymentsApiTest.php`
- `apps/openagents.com/tests/Feature/SparkWalletInvoicePayerTest.php`
- `apps/openagents.com/tests/Feature/L402PagesTest.php`

## 8) Recommended Next Increment (Post-EP212)

If we want agent-managed seller infra in Laravel:

1. Add authenticated/admin API for paywall lifecycle (create/update/delete) in `apps/openagents.com`.
2. Back it with an operator service that writes Aperture config and triggers deploy/sync safely.
3. Add receipt/events for paywall mutations (`l402_paywall_created`, etc.) and expose in `/api/l402/deployments`.
4. Add explicit tool(s) for controlled seller actions (separate from buyer fetch tools).
