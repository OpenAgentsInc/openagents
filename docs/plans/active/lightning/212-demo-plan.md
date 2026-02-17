# EP212 Demo Plan (Laravel `apps/openagents.com`)

Status: Active
Date: 2026-02-12
Last updated: 2026-02-17
Owner: OpenAgents

## Important Scope Note

This plan now targets the Laravel app at `apps/openagents.com`.

Old EP212 references to:

- `apps/web`
- Convex orchestration
- desktop payer/executor

are deprecated for current implementation planning.

For full implementation audit details, see:

- `docs/plans/active/lightning/212-demo-laravel-audit.md`

## 1) Demo Goal

Inside `openagents.com` chat, show a complete L402 buyer flow:

1. User asks for a paid fetch with a max spend cap.
2. Agent creates payment intent and requires explicit approval.
3. User approves.
4. Agent pays the invoice via Spark wallet backend, retries with L402 auth, and returns result.
5. UI/API surfaces show payment receipts and wallet/transaction status.
6. Repeat request demonstrates cache behavior.
7. Over-cap/disallowed domain path blocks pre-payment.

## 2) Current Implementation Summary (Laravel)

Implemented in `apps/openagents.com`:

- L402 chat tools:
  - `lightning_l402_fetch`
  - `lightning_l402_approve`
- Approval queue store + task lifecycle with TTL and user binding.
- L402 client flow:
  - allowlist
  - 402 challenge parse
  - quote/cap guardrail
  - invoice pay + retry
  - credential cache
  - bounded response capture (preview/hash)
- Per-user Spark wallet integration with API:
  - wallet provision/import
  - balance
  - create invoice
  - pay BOLT11
  - send spark
- L402 UI pages:
  - wallet, transactions, paywalls, settlements, deployments
- L402 read APIs:
  - `/api/l402/*`

## 3) Clarification: Paywall Creation Via API

Current answer: **No**.

What exists:

- `GET /api/l402/paywalls` (read-only aggregation from receipt history)

What does not exist:

- No API endpoint to create/update/delete paywalls from `apps/openagents.com`
- No agent tool in Laravel for provisioning seller paywall routes

Paywall lifecycle today remains an infra operation (Aperture/GCP config workflow), not an app API mutation.

## 4) Demo-Ready Sequence

Recommended EP212 recording sequence on current stack:

1. Open chat in `openagents.com`.
2. Ask for L402 paid call to approved endpoint with strict cap.
3. Show approval required state.
4. Approve.
5. Show paid completion + proof reference.
6. Open `/l402/transactions` and `/l402` (wallet) to show receipts and summary.
7. Repeat request to show cache behavior.
8. Run an over-cap/disallowed request to show pre-payment block.

## 5) Suggested Supporting API Demo (Optional)

If there is time, briefly show:

- `GET /api/l402/wallet`
- `GET /api/l402/transactions`
- `POST /api/agent-payments/invoice`
- `POST /api/agent-payments/pay`

This reinforces that the same Lightning state used by chat is available by API.

## 6) Acceptance Criteria (Laravel)

EP212 is ready when all are true:

1. Approval-gated L402 flow works in production chat.
2. A paid request returns successful response + receipt metadata.
3. Repeat request can demonstrate cache behavior.
4. Policy deny path clearly blocks pre-payment when cap/allowlist fails.
5. L402 pages reflect the same run receipt data used in chat.
6. OpenAPI and `/api/l402/*` endpoints reflect current behavior.

## 7) Follow-up After EP212

If we want agents to create seller paywalls directly from app/API:

1. Add authenticated/admin mutation endpoints for paywall lifecycle.
2. Add controlled operator service for Aperture config + deploy orchestration.
3. Add event logging and deployment receipts for paywall mutations.
4. Add dedicated agent tooling for seller operations.
