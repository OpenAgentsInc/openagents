# OpenClaw on Cloudflare (Moltworker notes + OpenAgents plan)

Context: Cloudflare published "Introducing Moltworker" (2026-01-29) describing how to run OpenClaw (formerly Moltbot) on Cloudflare Workers + Sandboxes, using Browser Rendering, R2, AI Gateway, and Access.

This doc maps that architecture to OpenAgents primitives and outlines what we can ship.

## The core idea (why it fits)

Cloudflare's pattern is:

- a thin **Worker** as the API router + auth boundary
- an isolated **Sandbox** container that runs the agent runtime
- **R2** mounted as persistent storage for the sandbox
- **Browser Rendering** as a managed headless browser capability
- **AI Gateway** as a model proxy (BYOK or unified billing)
- **Access** for authentication and policy

That is structurally aligned with OpenAgents:

- Runtime executes tools in sandboxes and records receipts.
- Budgets bound autonomy (spend limits + approvals).
- Identity should be portable (keys), not tied to a platform account.

## What we can offer (two options)

### Option A (preferred): "Bring your own Cloudflare account"

Ship an open-source reference deployment:

- user deploys OpenClaw/OpenAgents worker + sandbox into *their* Cloudflare account
- user supplies model keys (or uses AI Gateway unified billing)
- user owns storage (R2) and logs

This preserves "sovereign agent" posture:

- no custody of keys (identity or wallet)
- no platform lock-in (exportable state)
- clear isolation boundaries

### Option B (managed): "OpenClaw hosting" as a service

OpenAgents runs the worker + sandboxes for customers and bills for usage.

If we do this, keep it honest:

- we can provide uptime + operational ergonomics
- but customers should still be able to export state and rotate keys
- we should keep strong guardrails: budget caps, audit logs, explicit permissions

## How it composes with OpenAgents (interop direction)

### Identity + coordination (Nostr)

Make the hosted agent "speak" open coordination rails:

- publish a signed profile (NIP-SA AgentProfile kind 39200)
- accept job requests and emit results (NIP-90 patterns)
- coordinate privately over encrypted Nostr messaging (NIP-17/NIP-44/NIP-59)

The user can still control the agent via Slack/Discord/etc, but Nostr gives:

- portable identity
- open discovery
- encrypted agent-to-agent execution channels

### Money (Bitcoin/Lightning)

Hosted agents need payments to become economic actors:

- Lightning (fast settlement)
- budgets as the control plane (per-task hard caps)

OpenAgents already has Spark/Breez primitives (`crates/spark/`) and `UnifiedIdentity`.

## Concrete architecture: "OpenAgents Worker Lane" (Cloudflare)

We can treat Cloudflare as an execution lane/provider:

1) **Worker API**
   - endpoints: start/stop sandbox, run job, fetch logs, admin UI (optional)
   - auth: Cloudflare Access JWT
2) **Sandbox**
   - runs OpenClaw runtime (or OpenAgents runtime) in a container
   - tools: filesystem, command exec, background processes
3) **Storage**
   - R2 bucket mounted into sandbox for durable state (sessions, memory, receipts)
4) **Browser tool**
   - Browser Rendering used as the "browser lane"
   - expose via a tool schema so the runtime can call it deterministically
5) **Model access**
   - AI Gateway configured per tenant (BYOK or unified billing)
   - runtime uses provider base URL env var (no code changes in most cases)
6) **Receipts / audit**
   - every tool call emits: params_hash, output_hash, latency, side_effects
   - store receipts in R2 (append-only) + index in D1 (queryable)

## What openagents.com can do next (Cloudflare-native)

OpenAgents already runs on Cloudflare Pages.
We can expand with Workers for "agent infrastructure":

- `indexer.openagents.com` (Worker + D1/R2/KV/Queues) to index external ecosystems (e.g. Moltbook API)
- optional Nostr bridge publisher (mirror public posts to relays with receipts)
- wallet onboarding utilities (faucet + proof-of-control), gated and rate-limited

This keeps the website as the **canonical onboarding + docs** surface, while Workers handle durable ingestion/publishing workflows.

## Recommended ship sequence

1) Contribute to / integrate with Cloudflare's open-source Moltworker:
   - add OpenAgents "interop pack" (Nostr identity + job request/result + wallet hooks)
2) Stand up `indexer.openagents.com` for ingest + search + mirroring (see `docs/openclaw/bitcoin-wallets-plan.md`)
3) Add an OpenClaw "wallet onboarding" KB page (website) and a small starter-sats faucet (optional)
4) Decide whether we want managed hosting (Option B) after Option A proves demand

