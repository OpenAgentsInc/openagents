# Artanis — the OpenAgents cloud mind

Artanis is OpenAgents' **autonomous agent that runs in production** — the "cloud
mind" that keeps the network moving without a human in the loop. It lives inside
the `openagents.com` Cloudflare Worker, wakes on a **once-a-minute cron tick**,
and does real, bounded work each tick under a tested autonomous-loop contract.

This directory holds Artanis's design, audits, and runbooks (50+ dated docs).
This README is the orientation; start here, then follow the links.

## What it is

- **A cloud mind, not a chatbot.** The mind (`artanis-mind.ts`) runs Gemini
  inference through the Cloudflare AI Gateway (`openagents-ai-gateway`, BYOK,
  authenticated, with direct Google AI Studio as automatic fallback), admin-gated.
  Promise: `artanis.cloud_mind.v1` (**green**).
- **A scheduled runtime** (`artanis-scheduled-runner.ts`, `artanis-loop.ts`,
  `artanis-runtime.ts`): every minute a config-gated runner persists loop, tick,
  runtime, Forum-intent, and health records to D1 under the autonomous-loop
  contract (`2026-06-06-autonomous-loop-contract.md`). It is designed to **keep
  working and never sit idle**, coordinating over the Forum and Nostr.

## What it does each tick

- **Forum support responder** (`artanis-forum-responder.ts`,
  `artanis-reply-composer.ts`, `artanis-forum-delivery.ts`): scans new Forum
  topics, the mind classifies Pylon device/training questions via typed semantic
  selection, and composes replies **grounded only in the asker's post + the live
  promise registry**. Promise: `artanis.pylon_support_responder.v1` (yellow).
- **Payout dispatch** (`artanis-spend.ts`, `artanis-administrator-tick.ts`,
  `artanis-admin-closeout-receipts.ts`): dispatches **bounded** treasury Lightning
  payouts for accepted work under a hard **daily dispatch bound** and operator
  approval gates. This is the payout leg behind the Tassadar run's settlement.
- **Tassadar evolution loop** (`2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md`):
  the loop that drives the executor-trace training direction.
  Promise: `artanis.tassadar_evolution_loop.v1` (yellow).
- **Labor requester** (`artanis-labor-requester.ts`): a default-off surface to
  request labor-market work. Promise: `artanis.labor_requester.v1` (yellow).
- **GEPA scheduled runner**, **public report aggregation**
  (`artanis-public-report.ts`), **health/staleness monitor**
  (`artanis-health.ts`), and **work routing** (`artanis-work-routing.ts`).

## Authority boundary (what it can't do)

Artanis acts only within explicit, bounded authority:

- **Spend is capped** — a per-tick / daily dispatch bound; it cannot exceed the
  spend cap, approve payout targets, or move money outside the gated flow.
- **Operator approval gates** (`artanis-approval-gates.ts`,
  `artanis-operator-steering.ts`) sit in front of state-changing actions.
- It is an **org-operated** agent (slug `artanis`), so by the Tassadar run's own
  admission rule it does **not** count as independent contributor proof.
- Public output is receipt-first and redacted (`artanis-public-report-routes.ts`);
  no secrets, mnemonics, or private payment material in any public projection.

## Surfaces

- Public tick monitor: `GET /api/public/artanis/admin-ticks` (authority boundary,
  daily dispatch bound, dispatched-today, decisions, counts-by-state).
- Operator console + steering: `artanis-operator-console-routes.ts`.
- Persistence: D1 (`artanis-persistence.ts`, `2026-06-06-d1-persistence.md`).

## Key references in this directory

- Full status: `2026-06-10-artanis-pylon-tassadar-full-status-audit.md`,
  `2026-06-10-artanis-production-tick-and-tassadar-evolution-audit.md`.
- Implementation deep-dive: `2026-06-06-artanis-implementation-audit.md`.
- RLM composition: `2026-06-28-artanis-rlm-composition-architecture.md`
  covers the #6654 Artanis owner-chat RLM slice and the matching FRLM conductor
  projection primitives.
- Deploy readiness: `2026-06-07-artanis-deploy-readiness-full-audit.md`,
  `2026-06-06-production-launch-gate-runbook.md`.
- Treasury / payouts: `treasury-runbook.md`, `tips-buffer-runbook.md`.
- Autonomy contract: `2026-06-06-autonomous-loop-contract.md`,
  `2026-06-06-standalone-autonomy-claim-ledger.md`.

> Live promise states (authoritative): <https://openagents.com/api/public/product-promises>.
